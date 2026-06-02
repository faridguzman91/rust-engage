import { ref, onUnmounted } from "vue";
import { useMessagesStore } from "../stores/messages";
import type { Message } from "../stores/messages";

type WSStatus = "disconnected" | "connecting" | "connected";

let socket: WebSocket | null = null;
const status = ref<WSStatus>("disconnected");

export function useWebSocket(serverUrl: string) {
  const messagesStore = useMessagesStore();

  function connect() {
    if (socket?.readyState === WebSocket.OPEN) return;

    status.value = "connecting";
    socket = new WebSocket(serverUrl);

    socket.onopen = () => {
      status.value = "connected";
    };

    socket.onmessage = (event) => {
      try {
        const envelope = JSON.parse(event.data as string);
        if (envelope.type === "message") {
          messagesStore.append(envelope.payload as Message);
        }
      } catch {
        // malformed envelope — ignore
      }
    };

    socket.onclose = () => {
      status.value = "disconnected";
      // exponential backoff reconnect
      setTimeout(connect, 3000);
    };

    socket.onerror = () => {
      socket?.close();
    };
  }

  function disconnect() {
    socket?.close();
    socket = null;
    status.value = "disconnected";
  }

  function send(payload: unknown) {
    if (socket?.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify(payload));
    }
  }

  onUnmounted(disconnect);

  return { status, connect, disconnect, send };
}
