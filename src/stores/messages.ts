// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

// @faridguzman: Messages store — handles the full send/receive pipeline.
//
// Send pipeline (Phase 2 — optimistic + retry queue):
//   1. ensureSession()        — X3DH key agreement on first message to a contact
//   2. encrypt_message        — Double Ratchet (Rust); produces ciphertext for the network
//   3. send_message           — persist locally with status="sending"; returns Message + ID
//   4. update_message_status  — write "sending" to SQLite (overrides Tauri's default "sent")
//   5. queue_pending_message  — store the sealed envelope in pending_messages so it
//                               survives a crash or connectivity loss before step 6
//   6. sendEnvelope()         — POST to relay server
//      • Success → update_message_status("sent") + remove_pending_message + updateStatus("sent")
//      • Failure → updateStatus("failed"); row stays in pending_messages for drain()
//
// Drain pipeline (called on every WS reconnect):
//   list_pending_messages → retry each POST → on success remove row + mark "sent"
//                                           → on failure increment_pending_retry
//
// Receive pipeline (incoming WS push or offline drain):
//   useWebSocket.ts decrypts and calls append() which deduplicates by message ID.
//
// Disappearing messages:
//   Messages with expiresAt set are scheduled for removal by useDisappearingMessages.
//   remove() is called when the timer fires to purge from the store (DB deletion is
//   handled by sweep_expired_messages on the Rust side).
import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useContactsStore } from "./contacts";
import { useIdentityStore } from "./identity";
import { useServerApi } from "../composables/useServerApi";

export interface Message {
  id: string;
  conversationId: string;
  senderId: string;
  body: string;
  timestamp: number;
  status: "sending" | "sent" | "delivered" | "read" | "failed";
  isMine: boolean;
  expiresAt?: number | null;  // @faridguzman: Unix-ms expiry, null/undefined = never
}

// @faridguzman: Sealed envelope stored locally while awaiting successful relay.
// Fields mirror the pending_messages SQLite table in the Tauri side.
export interface PendingMessage {
  id: string;
  conversationId: string;
  recipientId: string;
  senderIk: string;
  ephemeralKey: string | null;
  ciphertext: string;
  timestamp: number;
  retryCount: number;
}

export const useMessagesStore = defineStore("messages", () => {
  // @faridguzman: Keyed by conversationId (= contactId) for O(1) lookup per thread
  const byConversation = ref<Record<string, Message[]>>({});

  async function load(conversationId: string) {
    const msgs = await invoke<Message[]>("list_messages", { conversationId });
    byConversation.value[conversationId] = msgs;
  }

  async function send(conversationId: string, body: string): Promise<Message> {
    const contacts = useContactsStore();
    const identity = useIdentityStore();
    const api = useServerApi();

    // 1. X3DH session setup (no-op after the first message)
    const ephemeralKey = await contacts.ensureSession(conversationId);

    // 2. Encrypt through the Double Ratchet — advances the ratchet chain key.
    //    We must NOT call encrypt again for this message after this point;
    //    the ciphertext stored here is the one that goes to the server and the queue.
    const encrypted = await invoke<{ ciphertext: string; messageType: number }>(
      "encrypt_message",
      { contactId: conversationId, plaintext: body }
    );

    const senderIk = identity.keys?.identityPublicKey ?? "";

    // 3. Persist locally so the message has a stable ID before the POST attempt.
    //    Tauri returns status="sent" by default; we immediately override to "sending"
    //    so the UI reflects the real in-flight state.
    const msg = await invoke<Message>("send_message", { conversationId, body });
    msg.status = "sending";

    // @faridguzman: Show the message in the UI immediately (optimistic update)
    append(msg);

    // 4. Persist "sending" to SQLite so status survives an app restart mid-flight.
    await invoke("update_message_status", { messageId: msg.id, status: "sending" }).catch(() => {});

    // 5. Queue the sealed envelope before attempting the POST.
    //    If the app crashes between here and step 6 the envelope is not lost —
    //    it will be drained on the next successful WebSocket connection.
    await invoke("queue_pending_message", {
      messageId: msg.id,
      conversationId,
      recipientId: conversationId,
      senderIk,
      ephemeralKey: ephemeralKey ?? null,
      ciphertext: encrypted.ciphertext,
      timestamp: msg.timestamp,
    }).catch(() => {}); // non-fatal; drain will pick it up if this fails

    // 6. POST the sealed envelope to the relay server
    try {
      await api.sendEnvelope({
        recipientId: conversationId,
        senderIk,
        ephemeralKey: ephemeralKey ?? undefined,
        ciphertext: encrypted.ciphertext,
      });

      // @faridguzman: Server accepted the envelope — mark as sent and remove from queue
      await invoke("update_message_status", { messageId: msg.id, status: "sent" }).catch(() => {});
      await invoke("remove_pending_message", { messageId: msg.id }).catch(() => {});
      updateStatus(msg.id, "sent");
    } catch {
      // @faridguzman: POST failed (offline, server down, etc.) — mark as failed in the
      // UI so the user knows.  The envelope stays in pending_messages and will be
      // retried automatically when the WebSocket reconnects (see drainPending).
      updateStatus(msg.id, "failed");
      await invoke("update_message_status", { messageId: msg.id, status: "failed" }).catch(() => {});
    }

    return msg;
  }

  // @faridguzman: Retry all queued envelopes that failed to reach the relay server.
  // Called on every successful WebSocket connection so messages sent while offline
  // are delivered as soon as connectivity is restored.
  async function drainPending(): Promise<void> {
    const api = useServerApi();
    let pending: PendingMessage[] = [];
    try {
      pending = await invoke<PendingMessage[]>("list_pending_messages");
    } catch {
      return; // nothing queued or DB unavailable
    }

    for (const p of pending) {
      try {
        await api.sendEnvelope({
          recipientId: p.recipientId,
          senderIk: p.senderIk,
          ephemeralKey: p.ephemeralKey ?? undefined,
          ciphertext: p.ciphertext,
        });

        // @faridguzman: Delivered — remove from queue and update status in DB + store
        await invoke("update_message_status", { messageId: p.id, status: "sent" }).catch(() => {});
        await invoke("remove_pending_message", { messageId: p.id }).catch(() => {});
        updateStatus(p.id, "sent");
      } catch {
        // @faridguzman: Still unreachable — bump the retry counter so we can surface
        // persistent failures (e.g. after MAX_RETRIES) to the user in a future pass.
        await invoke("increment_pending_retry", { messageId: p.id }).catch(() => {});
      }
    }
  }

  // @faridguzman: append() is called by the WS handler and offline drain.
  // Deduplicates by ID to guard against WS push racing with the REST fetch.
  function append(msg: Message) {
    if (!byConversation.value[msg.conversationId]) {
      byConversation.value[msg.conversationId] = [];
    }
    const exists = byConversation.value[msg.conversationId].some((m) => m.id === msg.id);
    if (!exists) {
      byConversation.value[msg.conversationId].push(msg);
    }
  }

  // @faridguzman: sendControl sends an encrypted control message (e.g. sender key
  // distribution) via the pairwise ratchet without creating a visible chat message.
  async function sendControl(contactId: string, payload: object): Promise<void> {
    const contacts = useContactsStore();
    const identity = useIdentityStore();
    const api = useServerApi();

    const ephemeralKey = await contacts.ensureSession(contactId);
    const body = JSON.stringify({ __control: true, ...payload });
    const encrypted = await invoke<{ ciphertext: string; messageType: number }>(
      "encrypt_message",
      { contactId, plaintext: body }
    );
    await api.sendEnvelope({
      recipientId: contactId,
      senderIk: identity.keys?.identityPublicKey ?? "",
      ephemeralKey: ephemeralKey ?? undefined,
      ciphertext: encrypted.ciphertext,
    });
  }

  // @faridguzman: Update the delivery status of a sent message in-memory.
  // Called by the WS handler when the server forwards an ack (delivered) or
  // read receipt back to us. We scan all conversations since the WS envelope
  // only carries the message ID, not the conversation ID.
  function updateStatus(messageId: string, status: Message["status"]) {
    for (const msgs of Object.values(byConversation.value)) {
      const msg = msgs.find((m) => m.id === messageId);
      if (msg) {
        msg.status = status;
        return;
      }
    }
  }

  // @faridguzman: Remove a single message from the in-memory store.
  // Called by useDisappearingMessages when a timer fires.
  // DB deletion is handled separately by sweep_expired_messages.
  function remove(messageId: string, conversationId: string) {
    const msgs = byConversation.value[conversationId];
    if (msgs) {
      byConversation.value[conversationId] = msgs.filter((m) => m.id !== messageId);
    }
  }

  function forConversation(id: string): Message[] {
    return byConversation.value[id] ?? [];
  }

  return { byConversation, load, send, sendControl, append, remove, forConversation, updateStatus, drainPending };
});
