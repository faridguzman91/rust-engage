// @faridguzman91: Messages store — handles the full send/receive pipeline.
//
// Send pipeline:
//   1. ensureSession()  — X3DH key agreement on first message to a contact
//   2. encrypt_message  — Double Ratchet encryption (Rust side, returns ciphertext JSON)
//   3. sendEnvelope()   — POST sealed envelope to the relay server (server sees only ciphertext)
//   4. send_message     — persist locally and return to the UI (plaintext for display)
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
  expiresAt?: number | null;  // @faridguzman91: Unix-ms expiry, null/undefined = never
}

export const useMessagesStore = defineStore("messages", () => {
  // @faridguzman91: Keyed by conversationId (= contactId) for O(1) lookup per thread
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

    // 2. Encrypt through the Double Ratchet — each message uses a unique chain key
    const encrypted = await invoke<{ ciphertext: string; messageType: number }>(
      "encrypt_message",
      { contactId: conversationId, plaintext: body }
    );

    // 3. POST sealed envelope — relay server is a zero-knowledge forwarder
    await api.sendEnvelope({
      recipientId: conversationId,
      senderIk: identity.keys?.identityPublicKey ?? "",
      ephemeralKey: ephemeralKey ?? undefined,
      ciphertext: encrypted.ciphertext,
    });

    // 4. Persist locally and return to UI (Rust sets expires_at if timer is active)
    const msg = await invoke<Message>("send_message", { conversationId, body });

    if (!byConversation.value[conversationId]) {
      byConversation.value[conversationId] = [];
    }
    byConversation.value[conversationId].push(msg);
    return msg;
  }

  // @faridguzman91: append() is called by the WS handler and offline drain.
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

  // @faridguzman91: sendControl sends an encrypted control message (e.g. sender key
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

  // @faridguzman91: Update the delivery status of a sent message in-memory.
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

  // @faridguzman91: Remove a single message from the in-memory store.
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

  return { byConversation, load, send, sendControl, append, remove, forConversation, updateStatus };
});
