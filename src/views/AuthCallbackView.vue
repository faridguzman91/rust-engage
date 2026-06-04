<script setup lang="ts">
import { onMounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useAuthStore } from "../stores/auth";
import ProgressSpinner from "primevue/progressspinner";

const router = useRouter();
const route = useRoute();
const auth = useAuthStore();

onMounted(() => {
  const token = route.query.token as string | undefined;

  if (token) {
    auth.setToken(token);
    // First time user — go to setup; returning user — go straight to chat
    router.replace(auth.profile ? "/setup" : "/setup");
  } else {
    // No token in URL — something went wrong, back to login
    router.replace("/login");
  }
});
</script>

<template>
  <div class="callback-wrap">
    <ProgressSpinner style="width: 48px; height: 48px;" stroke-width="4" />
    <p>Signing you in…</p>
  </div>
</template>

<style scoped>
.callback-wrap {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  gap: 1rem;
  background: var(--engage-main-bg);
  color: var(--engage-muted);
  font-size: 0.9rem;
}
</style>
