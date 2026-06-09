// @faridguzman: WebSocket singleton — one persistent connection per app session.
// Handles:
//   - JWT auth via ?token= query param (WebSocket handshakes can't send headers)
//   - Incoming message decryption via Tauri invoke (X3DH inbound session init + Double Ratchet)
//   - Exponential backoff reconnect (1s → 30s cap)
//   - OPK replenishment check on every successful connect
//   - ACK back to server after each delivered message
//   - Sequence-number gap detection: if seqNum > lastSeq+1 the client drains the
//     server's offline queue to recover any missed messages
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useMessagesStore } from "../stores/messages";
import type { Message } from "../stores/messages";
import { SERVER_WS } from "../config";
import { useOpkReplenishment } from "./useOpkReplenishment";
import { useDisappearingMessages } from "./useDisappearingMessages";
import { useGroupsStore } from "../stores/groups";
import { useServerApi } from "./useServerApi";

// ── Sequence tracking ─────────────────────────────────────────────────────────
// @faridguzman: lastSeq is the highest sequence number we have successfully
// processed.  Persisted in localStorage so gaps spanning an app restart are
// also detected.  A gap triggers an immediate offline drain.

const SEQ_KEY = "engage_last_seq";

function getLastSeq(): number {
  return parseInt(localStorage.getItem(SEQ_KEY) ?? "0", 10) || 0;
}

function setLastSeq(seq: number): void {
  localStorage.setItem(SEQ_KEY, String(seq));
}

// @faridguzman: Check if seqNum from an incoming envelope reveals a gap.
// Returns true if a drain should be triggered.
function checkSeq(seqNum: number | null | undefined): boolean {
  if (!seqNum) return false; // pre-Phase-3 message or no seq — ignore
  const last = getLastSeq();
  const gap = seqNum > last + 1;
  if (seqNum > last) setLastSeq(seqNum);
  return gap;
}

function getToken(): string {
  return localStorage.getItem("engage_jwt") ?? "";
}

type WSStatus = "disconnected" | "connecting" | "connected";

// @faridguzman: Module-level singleton so all components share one socket instance
let socket: WebSocket | null = null;
let userId: string | null = null;
let retryDelay = 1000;
const MAX_DELAY = 30_000;

const status = ref<WSStatus>("disconnected");

// ── Shared message processors ─────────────────────────────────────────────────
// @faridguzman: Extracted so the live WS path and the gap-drain path both use
// the same decrypt-then-append logic without duplication.

type Raw1to1 = {
  id: string; senderId: string; senderIk: string;
  ephemeralKey?: string; ciphertext: string; timestamp: number; seqNum?: number;
};

type RawGroup = {
  id: string; groupId: string; senderId: string; senderIk: string;
  ciphertext: string; timestamp: number; seqNum?: number;
};

async function processIncoming1to1(
  raw: Raw1to1,
  messagesStore: ReturnType<typeof useMessagesStore>,
  send: (p: unknown) => void,
) {
  // @faridguzman: First message from this sender includes ephemeralKey (X3DH EK_A).
  // We must init the inbound session before decrypting.
  if (raw.ephemeralKey) {
    try {
      await invoke("init_inbound_session", {
        contactId: raw.senderId,
        senderIk: raw.senderIk,
        ephemeralKey: raw.ephemeralKey,
      });
    } catch {
      // session already exists or init failed — attempt decrypt anyway
    }
  }

  let body: string;
  try {
    body = await invoke<string>("decrypt_message", {
      contactId: raw.senderId,
      ciphertext: raw.ciphertext,
      messageType: 1,
    });
  } catch {
    body = "[encrypted message]";
  }

  const msg: Message = {
    id: raw.id,
    conversationId: raw.senderId,
    senderId: raw.senderId,
    body,
    timestamp: raw.timestamp,
    status: "delivered",
    isMine: false,
  };
  messagesStore.append(msg);

  // @faridguzman: Schedule disappearing-message timer if expiresAt is set
  if (msg.expiresAt) {
    await invoke("set_message_expiry", {
      messageId: msg.id,
      expiresAt: msg.expiresAt,
    }).catch(() => {});
    useDisappearingMessages().scheduleExpiry(msg);
  }

  send({ type: "ack", messageId: raw.id });
}

async function processIncomingGroup(
  raw: RawGroup,
  messagesStore: ReturnType<typeof useMessagesStore>,
  send: (p: unknown) => void,
) {
  const groups = useGroupsStore();
  let body: string;
  try {
    body = await groups.decryptMessage(raw.groupId, raw.senderId, raw.ciphertext);
  } catch {
    body = "[encrypted group message]";
  }

  // @faridguzman: Control messages (Sender Key distribution) are not displayed
  try {
    const ctrl = JSON.parse(body);
    if (ctrl.__control && ctrl.type === "sender_key_distribution") {
      await groups.receiveDistribution(ctrl.payload);
      return;
    }
  } catch { /* not JSON — regular message */ }

  messagesStore.append({
    id: raw.id,
    conversationId: raw.groupId,
    senderId: raw.senderId,
    body,
    timestamp: raw.timestamp,
    status: "delivered",
    isMine: false,
  });
  send({ type: "ack", messageId: raw.id });
}

// @faridguzman: Pull the server's offline queue and process every pending
// message through the same decrypt pipeline.  Called whenever gap detection
// fires — the missed message should still be in the server queue (delivered=0).
async function drainMissed(
  messagesStore: ReturnType<typeof useMessagesStore>,
  send: (p: unknown) => void,
) {
  const userId = localStorage.getItem("engage_user_id");
  if (!userId) return;

  const api = useServerApi();
  let missed: unknown[];
  try {
    missed = await api.fetchPendingMessages(userId);
  } catch {
    return;
  }

  for (const raw of missed) {
    const m = raw as Raw1to1 & { groupId?: string };
    if (m.groupId) {
      await processIncomingGroup(m as unknown as RawGroup, messagesStore, send);
    } else {
      await processIncoming1to1(m, messagesStore, send);
    }
    if (m.seqNum) setLastSeq(Math.max(getLastSeq(), m.seqNum));
  }
}

export function useWebSocket() {
  const messagesStore = useMessagesStore();

  function connect(uid: string) {
    userId = uid;
    _connect();
  }

  function _connect() {
    if (!userId) return;
    if (socket?.readyState === WebSocket.OPEN) return;

    status.value = "connecting";
    const token = encodeURIComponent(getToken());
    socket = new WebSocket(`${SERVER_WS}/ws/${encodeURIComponent(userId!)}?token=${token}`);

    socket.onopen = () => {
      status.value = "connected";
      retryDelay = 1000; // reset backoff on successful connect
      // @faridguzman: Check OPK pool silently on every (re)connect
      useOpkReplenishment().checkAndReplenish();
      // @faridguzman: Drain any envelopes that failed to send while offline.
      // Runs async — does not block the WS message loop.
      messagesStore.drainPending().catch(() => {});
    };

    socket.onmessage = async (event) => {
      let envelope: { type: string; payload?: unknown };
      try {
        envelope = JSON.parse(event.data as string);
      } catch {
        return;
      }

      // ── Delivery receipt — server forwarded recipient's ACK back to us ──────
      if (envelope.type === "ack") {
        const { message_id } = envelope as unknown as { message_id: string };
        messagesStore.updateStatus(message_id, "delivered");
        // Persist the new status to local SQLite so it survives a restart
        invoke("update_message_status", { messageId: message_id, status: "delivered" }).catch(() => {});
        return;
      }

      // ── Read receipt — recipient opened the conversation ──────────────────
      if (envelope.type === "read") {
        const { message_id } = envelope as unknown as { message_id: string };
        messagesStore.updateStatus(message_id, "read");
        invoke("update_message_status", { messageId: message_id, status: "read" }).catch(() => {});
        return;
      }

      if (envelope.type === "message") {
        const raw = envelope.payload as {
          id: string;
          senderId: string;
          senderIk: string;
          ephemeralKey?: string;
          ciphertext: string;
          timestamp: number;
          seqNum?: number;
        };

        await processIncoming1to1(raw, messagesStore, send);

        // @faridguzman: Gap detection — if this seq is not the one we expected,
        // pull the server's offline queue to recover missed messages.
        if (checkSeq(raw.seqNum)) {
          drainMissed(messagesStore, send).catch(() => {});
        }
      }

      // @faridguzman: Group message — decrypt with the sender's Sender Key
      if (envelope.type === "group_message") {
        const raw = envelope.payload as {
          id: string;
          groupId: string;
          senderId: string;
          senderIk: string;
          ciphertext: string;
          timestamp: number;
          seqNum?: number;
        };

        await processIncomingGroup(raw, messagesStore, send);

        if (checkSeq(raw.seqNum)) {
          drainMissed(messagesStore, send).catch(() => {});
        }
      }
    };

    socket.onclose = () => {
      status.value = "disconnected";
      // @faridguzman: Exponential backoff — doubles each attempt, capped at 30s
      setTimeout(() => {
        retryDelay = Math.min(retryDelay * 2, MAX_DELAY);
        _connect();
      }, retryDelay);
    };

    socket.onerror = () => {
      socket?.close();
    };
  }

  function disconnect() {
    const s = socket;
    socket = null;
    userId = null;
    s?.close();
    status.value = "disconnected";
  }

  function send(payload: unknown) {
    if (socket?.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify(payload));
    }
  }

  // @faridguzman: Emit "read" receipts for every received message in a
  // conversation. Called when the user opens a thread (and on each new
  // incoming message while the thread is visible). The server looks up the
  // original sender and forwards a WsEnvelope::Read to them.
  function markRead(conversationId: string) {
    const msgs = messagesStore.forConversation(conversationId);
    for (const msg of msgs) {
      if (!msg.isMine && msg.status !== "read") {
        send({ type: "read", messageId: msg.id });
      }
    }
  }

  return { status, connect, disconnect, send, markRead };
}
