import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface Message {
  id: string;
  conversationId: string;
  senderId: string;
  body: string;
  timestamp: number;
  status: "sending" | "sent" | "delivered" | "read" | "failed";
  isMine: boolean;
}

export const useMessagesStore = defineStore("messages", () => {
  const byConversation = ref<Record<string, Message[]>>({});

  async function load(conversationId: string) {
    const msgs = await invoke<Message[]>("list_messages", { conversationId });
    byConversation.value[conversationId] = msgs;
  }

  async function send(conversationId: string, body: string): Promise<Message> {
    const msg = await invoke<Message>("send_message", { conversationId, body });
    if (!byConversation.value[conversationId]) {
      byConversation.value[conversationId] = [];
    }
    byConversation.value[conversationId].push(msg);
    return msg;
  }

  function append(msg: Message) {
    if (!byConversation.value[msg.conversationId]) {
      byConversation.value[msg.conversationId] = [];
    }
    byConversation.value[msg.conversationId].push(msg);
  }

  function forConversation(id: string): Message[] {
    return byConversation.value[id] ?? [];
  }

  return { byConversation, load, send, append, forConversation };
});
