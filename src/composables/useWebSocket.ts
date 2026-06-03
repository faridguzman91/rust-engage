// @faridguzman91: WebSocket singleton — one persistent connection per app session.
// Handles:
//   - JWT auth via ?token= query param (WebSocket handshakes can't send headers)
//   - Incoming message decryption via Tauri invoke (X3DH inbound session init + Double Ratchet)
//   - Exponential backoff reconnect (1s → 30s cap)
//   - OPK replenishment check on every successful connect
//   - ACK back to server after each delivered message
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useMessagesStore } from "../stores/messages";
import type { Message } from "../stores/messages";
import { SERVER_WS } from "../config";
import { useOpkReplenishment } from "./useOpkReplenishment";
import { useDisappearingMessages } from "./useDisappearingMessages";
import { useGroupsStore } from "../stores/groups";

function getToken(): string {
  return localStorage.getItem("engage_jwt") ?? "";
}

type WSStatus = "disconnected" | "connecting" | "connected";

// @faridguzman91: Module-level singleton so all components share one socket instance
let socket: WebSocket | null = null;
let userId: string | null = null;
let retryDelay = 1000;
const MAX_DELAY = 30_000;

const status = ref<WSStatus>("disconnected");

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
      // @faridguzman91: Check OPK pool silently on every (re)connect
      useOpkReplenishment().checkAndReplenish();
    };

    socket.onmessage = async (event) => {
      let envelope: { type: string; payload?: unknown };
      try {
        envelope = JSON.parse(event.data as string);
      } catch {
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
        };

        // @faridguzman91: First message from this sender includes ephemeralKey (X3DH EK_A).
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

        // Decrypt via Double Ratchet
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

        // @faridguzman91: If the sender has set a disappear timer, set expiry
        // on the inbound message and schedule its removal
        if (msg.expiresAt) {
          await invoke("set_message_expiry", {
            messageId: msg.id,
            expiresAt: msg.expiresAt,
          }).catch(() => {});
          useDisappearingMessages().scheduleExpiry(msg);
        }

        // ACK delivery back to server
        send({ type: "ack", messageId: raw.id });
      }

      // @faridguzman91: Group message — decrypt with the sender's Sender Key
      if (envelope.type === "group_message") {
        const raw = envelope.payload as {
          id: string;
          groupId: string;
          senderId: string;
          senderIk: string;
          ciphertext: string;
          timestamp: number;
        };

        const groups = useGroupsStore();
        let body: string;
        try {
          body = await groups.decryptMessage(raw.groupId, raw.senderId, raw.ciphertext);
        } catch {
          body = "[encrypted group message]";
        }

        // @faridguzman91: Check if this is a control message (Sender Key distribution)
        try {
          const ctrl = JSON.parse(body);
          if (ctrl.__control && ctrl.type === "sender_key_distribution") {
            await groups.receiveDistribution(ctrl.payload);
            return; // don't display control messages
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
    };

    socket.onclose = () => {
      status.value = "disconnected";
      // @faridguzman91: Exponential backoff — doubles each attempt, capped at 30s
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

  return { status, connect, disconnect, send };
}
