import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { SERVER_BASE } from "../config";

const TOKEN_KEY = "engage_jwt";

export const useAuthStore = defineStore("auth", () => {
  const token = ref<string | null>(localStorage.getItem(TOKEN_KEY));
  const isAuthenticated = computed(() => token.value !== null);

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

  /**
   * Start Google OAuth by navigating the Tauri webview directly.
   * The server will redirect back to http://localhost:1420/#/auth?token=JWT
   * so the webview returns to the app with __TAURI_INTERNALS__ still available.
   */
  function loginWithGoogle() {
    window.location.href = `${SERVER_BASE}/api/auth/google`;
  }

  return { token, isAuthenticated, profile, setToken, clearToken, loginWithGoogle };
});
