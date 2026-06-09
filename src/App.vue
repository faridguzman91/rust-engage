<script setup lang="ts">
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useRouter } from "vue-router";
import Toast from "primevue/toast";
import { useAuthStore } from "./stores/auth";
import { useIdentityStore } from "./stores/identity";
import { useMessagesStore } from "./stores/messages";
import { useServerApi } from "./composables/useServerApi";
import { onOpenedWith } from "@tauri-apps/plugin-deep-link";

const router = useRouter();
const auth = useAuthStore();
const identity = useIdentityStore();
const messagesStore = useMessagesStore();

// @faridguzman: Route an engage:// deep link to the correct Vue route.
// engage://invite?token=TOKEN  → /#/invite?token=TOKEN
// engage://auth?token=JWT      → /#/auth?token=JWT  (OAuth callback in production)
function handleDeepLink(url: string) {
  try {
    const parsed = new URL(url);
    const host = parsed.host; // e.g. "invite" or "auth"
    const token = parsed.searchParams.get("token");
    if (!token) return;
    if (host === "invite") router.push(`/invite?token=${encodeURIComponent(token)}`);
    if (host === "auth")   router.push(`/auth?token=${encodeURIComponent(token)}`);
  } catch {
    // malformed URL — ignore
  }
}

onMounted(async () => {
  // Enable PrimeVue dark mode
  document.documentElement.classList.add("dark");

  // @faridguzman: Register the deep link listener so invite and auth links
  // work while the app is already running (not just on cold launch).
  onOpenedWith((urls) => {
    for (const url of urls) handleDeepLink(url);
  }).catch(() => {}); // silently ignore if plugin not available (browser dev mode)

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
