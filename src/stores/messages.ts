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
}

export const useMessagesStore = defineStore("messages", () => {
  const byConversation = ref<Record<string, Message[]>>({});

  async function load(conversationId: string) {
    const msgs = await invoke<Message[]>("list_messages", { conversationId });
    byConversation.value[conversationId] = msgs;
  }

  async function send(conversationId: string, body: string): Promise<Message> {
    const contacts = useContactsStore();
    const identity = useIdentityStore();
    const api = useServerApi();

    // 1. Establish X3DH session with contact if not yet done.
    //    ensureSession returns the ephemeral key on first call (null after that).
    const ephemeralKey = await contacts.ensureSession(conversationId);

    // 2. Encrypt through the Double Ratchet (returns ratchet ciphertext JSON)
    const encrypted = await invoke<{ ciphertext: string; messageType: number }>(
      "encrypt_message",
      { contactId: conversationId, plaintext: body }
    );

    // 3. Relay sealed envelope to server — server is an opaque forwarder
    await api.sendEnvelope({
      recipientId: conversationId,
      senderIk: identity.keys?.identityPublicKey ?? "",
      ephemeralKey: ephemeralKey ?? undefined,
      ciphertext: encrypted.ciphertext,
    });

    // 4. Persist locally and return to UI
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
    const exists = byConversation.value[msg.conversationId].some((m) => m.id === msg.id);
    if (!exists) {
      byConversation.value[msg.conversationId].push(msg);
    }
  }

  function forConversation(id: string): Message[] {
    return byConversation.value[id] ?? [];
  }

  return { byConversation, load, send, append, forConversation };
});
