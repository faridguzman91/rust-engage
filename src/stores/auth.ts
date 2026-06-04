// @faridguzman91: Auth store — holds the JWT issued after Google OAuth.
// Token is persisted in localStorage so the user stays logged in across restarts.
//
// OAuth flow (dev):
//   1. loginWithGoogle() navigates the Tauri webview to the server's /api/auth/google
//   2. Server redirects to Google, user consents
//   3. Server exchanges code → issues HS256 JWT → redirects to localhost:1420/#/auth?token=JWT
//   4. AuthCallbackView extracts the token and calls setToken()
//
// We navigate the webview directly (window.location.href) rather than opening the system
// browser so that __TAURI_INTERNALS__ is always available after the redirect.
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { SERVER_BASE } from "../config";

const TOKEN_KEY = "engage_jwt";

export const useAuthStore = defineStore("auth", () => {
  const token = ref<string | null>(localStorage.getItem(TOKEN_KEY));
  const isAuthenticated = computed(() => token.value !== null);

  // @faridguzman91: Parse JWT payload client-side for display only — no signature
  // verification here. The server validates the signature on every API request.
  const profile = computed(() => {
    if (!token.value) return null;
    try {
      const payload = JSON.parse(atob(token.value.split(".")[1]));
      return { userId: payload.sub as string, email: payload.email as string };
    } catch {
      return null;
    }
  });

  function setToken(t: string) {
    token.value = t;
    localStorage.setItem(TOKEN_KEY, t);
  }

  function clearToken() {
    token.value = null;
    localStorage.removeItem(TOKEN_KEY);
  }

  // @faridguzman91: Navigate the Tauri webview through Google OAuth.
  // Must NOT use openUrl() here — that opens the system browser which has no
  // __TAURI_INTERNALS__, making invoke() calls fail on any subsequent screen.
  function loginWithGoogle() {
    window.location.href = `${SERVER_BASE}/api/auth/google`;
  }

  return { token, isAuthenticated, profile, setToken, clearToken, loginWithGoogle };
});
