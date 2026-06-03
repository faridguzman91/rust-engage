import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useMessagesStore } from "../stores/messages";
import type { Message } from "../stores/messages";
import { SERVER_WS } from "../config";
import { useOpkReplenishment } from "./useOpkReplenishment";

function getToken(): string {
  return localStorage.getItem("engage_jwt") ?? "";
}

type WSStatus = "disconnected" | "connecting" | "connected";

// Module-level singleton so all components share one socket
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
      // Check OPK pool every time we (re)connect — silent, non-blocking
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

        // If this is a first message (has ephemeralKey), establish inbound session first
        if (raw.ephemeralKey) {
          try {
            await invoke("init_inbound_session", {
              contactId: raw.senderId,
              senderIk: raw.senderIk,
              ephemeralKey: raw.ephemeralKey,
            });
          } catch {
            // session already exists or failed — try to decrypt anyway
          }
        }

        // Decrypt the ratchet payload
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

        // ACK back to server
        send({ type: "ack", messageId: raw.id });
      }
    };

    socket.onclose = () => {
      status.value = "disconnected";
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
