<script setup lang="ts">
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useIdentityStore } from "./stores/identity";
import { useMessagesStore } from "./stores/messages";
import { useServerApi } from "./composables/useServerApi";

const identity = useIdentityStore();
const messagesStore = useMessagesStore();

onMounted(async () => {
  await identity.initialize();

  // If we have an identity, drain any messages that arrived while offline
  if (identity.isSetup) {
    try {
      const api = useServerApi();
      const pending = await api.fetchPendingMessages(identity.userId);
      for (const raw of pending as any[]) {
        // Establish inbound session if this is a first-message envelope
        if (raw.ephemeralKey) {
          await invoke("init_inbound_session", {
            contactId: raw.senderId,
            senderIk: raw.senderIk,
            ephemeralKey: raw.ephemeralKey,
          }).catch(() => {});
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
        messagesStore.append({
          id: raw.id,
          conversationId: raw.senderId,
          senderId: raw.senderId,
          body,
          timestamp: raw.timestamp,
          status: "delivered",
          isMine: false,
        });
      }
    } catch {
      // server unreachable — offline mode, local messages still available
    }
  }
});
</script>

<template>
  <router-view />
</template>
