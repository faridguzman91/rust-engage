import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { SERVER_BASE } from "../config";

const TOKEN_KEY = "engage_jwt";

export const useAuthStore = defineStore("auth", () => {
  const token = ref<string | null>(localStorage.getItem(TOKEN_KEY));
  const isAuthenticated = computed(() => token.value !== null);

  // Parse the JWT payload (no signature verification — server does that)
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

  /** Open the system browser to start Google OAuth. */
  async function loginWithGoogle() {
    await openUrl(`${SERVER_BASE}/api/auth/google`);
  }

  /**
   * Register a one-time deep-link listener.
   * Called once on app startup — resolves with the token when OAuth redirects back.
   */
  function listenForDeepLink(): Promise<string> {
    return new Promise((resolve) => {
      onOpenUrl((urls) => {
        for (const url of urls) {
          const match = url.match(/[?&]token=([^&]+)/);
          if (match) {
            const t = decodeURIComponent(match[1]);
            setToken(t);
            resolve(t);
            return;
          }
        }
      });
    });
  }

  return { token, isAuthenticated, profile, setToken, clearToken, loginWithGoogle, listenForDeepLink };
});
