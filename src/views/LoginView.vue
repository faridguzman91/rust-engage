<script setup lang="ts">
import { ref } from "vue";
import { useAuthStore } from "../stores/auth";
import Card from "primevue/card";
import Button from "primevue/button";

const auth = useAuthStore();
const loading = ref(false);

function login() {
  loading.value = true;
  // Navigate the Tauri webview through Google OAuth.
  // The server redirects back to localhost:1420/#/auth?token=JWT so we
  // return to the app inside Tauri with __TAURI_INTERNALS__ available.
  auth.loginWithGoogle();
}
</script>

<template>
  <div class="login-wrap">
    <Card class="login-card">
      <template #content>
        <div class="login-body">
          <img src="/engage.svg" alt="engage" class="logo" />

          <div class="text-center">
            <p class="tagline">End-to-end encrypted messaging</p>
          </div>

          <Button
            class="google-btn"
            :loading="loading"
            :label="loading ? 'Opening browser…' : 'Continue with Google'"
            severity="secondary"
            outlined
            @click="login"
          >
            <template #icon>
              <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg" style="flex-shrink:0">
                <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" fill="#4285F4"/>
                <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853"/>
                <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l3.66-2.84z" fill="#FBBC05"/>
                <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335"/>
              </svg>
            </template>
          </Button>

          <p class="hint">
            Your messages are encrypted on your device.<br />
            Google is only used to verify your identity.
          </p>
        </div>
      </template>
    </Card>
  </div>
</template>

<style scoped>
.login-wrap {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  background: var(--engage-main-bg);
}
.login-card {
  width: 380px;
  background: var(--engage-sidebar-bg) !important;
  border: 1px solid var(--engage-border) !important;
  border-radius: 16px !important;
}
.login-body {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.25rem;
  padding: 0.5rem 0;
}
.logo { width: 240px; }
.tagline { color: var(--engage-muted); font-size: 0.9rem; margin: 0; }
.google-btn {
  width: 100%;
  justify-content: center;
  gap: 0.75rem;
}
.hint {
  font-size: 0.75rem;
  color: var(--engage-muted);
  text-align: center;
  line-height: 1.6;
}
</style>
