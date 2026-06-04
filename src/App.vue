<script setup lang="ts">
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import Toast from "primevue/toast";
import { useAuthStore } from "./stores/auth";
import { useIdentityStore } from "./stores/identity";
import { useMessagesStore } from "./stores/messages";
import { useServerApi } from "./composables/useServerApi";

const auth = useAuthStore();
const identity = useIdentityStore();
const messagesStore = useMessagesStore();

onMounted(async () => {
  // Enable PrimeVue dark mode
  document.documentElement.classList.add("dark");

  // Load existing identity if already authenticated
  if (auth.isAuthenticated) {
    await identity.initialize();

    // Drain offline messages
    if (identity.isSetup && auth.profile) {
      try {
        const api = useServerApi();
        const pending = await api.fetchPendingMessages(auth.profile.userId);
        for (const raw of pending as any[]) {
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
        // server unreachable — offline mode
      }
    }
  }
});
</script>

<template>
  <Toast position="bottom-right" />
  <router-view />
</template>
