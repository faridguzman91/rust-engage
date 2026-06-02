<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { useAuthStore } from "../stores/auth";

const router = useRouter();
const auth = useAuthStore();

const loading = ref(false);
const error = ref("");

async function login() {
  loading.value = true;
  error.value = "";
  try {
    // Open browser, then wait for the deep-link callback
    await Promise.all([
      auth.loginWithGoogle(),
      auth.listenForDeepLink(),
    ]);
    router.push(auth.isAuthenticated ? "/setup" : "/");
  } catch (e) {
    error.value = String(e);
    loading.value = false;
  }
}
</script>

<template>
  <div class="login-view">
    <div class="login-card">
      <p class="logo-wrap">
        <img src="/engage-logo.png" alt="engage" class="logo-img" onerror="this.style.display='none'" />
      </p>
      <h1 class="app-name">engage</h1>
      <p class="tagline">End-to-end encrypted messaging</p>

      <button class="google-btn" :disabled="loading" @click="login">
        <svg class="google-icon" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
          <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" fill="#4285F4"/>
          <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853"/>
          <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l3.66-2.84z" fill="#FBBC05"/>
          <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335"/>
        </svg>
        {{ loading ? "Opening browser…" : "Continue with Google" }}
      </button>

      <p v-if="error" class="error">{{ error }}</p>

      <p class="hint">
        Your messages are encrypted on your device.<br />
        Google is only used to verify your identity.
      </p>
    </div>
  </div>
</template>

<style scoped>
.login-view {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  background: var(--bg-primary);
}
.login-card {
  width: 360px;
  padding: 2.5rem 2rem;
  background: var(--bg-secondary);
  border-radius: 12px;
  box-shadow: 0 4px 24px rgba(0,0,0,0.15);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
}
.logo-wrap { margin: 0; }
.logo-img { width: 180px; }
.app-name {
  font-size: 1.8rem;
  font-weight: 700;
  color: var(--accent);
  margin: 0;
}
.tagline {
  color: var(--text-muted);
  font-size: 0.9rem;
  margin: 0;
}
.google-btn {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  width: 100%;
  padding: 0.7rem 1rem;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-input);
  color: var(--text-primary);
  font-size: 0.95rem;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s;
  justify-content: center;
}
.google-btn:hover:not(:disabled) { background: var(--bg-hover); }
.google-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.google-icon { width: 20px; height: 20px; flex-shrink: 0; }
.error { color: var(--danger); font-size: 0.85rem; text-align: center; }
.hint {
  font-size: 0.75rem;
  color: var(--text-muted);
  text-align: center;
  line-height: 1.5;
}
</style>
