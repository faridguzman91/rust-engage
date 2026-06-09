// @faridguzman: Identity store — manages the user's local crypto keys.
// On first run (after OAuth) the user picks a display name and this store:
//   1. Generates Ed25519 + X25519 key pairs locally via Tauri (never leaves the device unencrypted)
//   2. Registers the public keys with the relay server
//   3. Opens the WebSocket connection for real-time delivery
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useServerApi } from "../composables/useServerApi";
import { useWebSocket } from "../composables/useWebSocket";
import { useAuthStore } from "./auth";

// @faridguzman: Re-export userId as a module-level computed so groups.ts can
// access it without importing the whole store (avoids circular imports).
export function useUserId(): string {
  const auth = useAuthStore();
  return auth.profile?.userId ?? "";
}

export interface IdentityKeys {
  identityPublicKey: string;
  signedPreKeyPublicKey: string;
  registrationId: number;
}

export const useIdentityStore = defineStore("identity", () => {
  const keys = ref<IdentityKeys | null>(null);
  const displayName = ref<string>("");

  // @faridguzman: isSetup drives the router guard — both keys AND displayName must be
  // present before the user is allowed past the /setup route.
  const isSetup = computed(() => keys.value !== null && displayName.value !== "");

  async function initialize() {
    try {
      const result = await invoke<{ keys: IdentityKeys; displayName: string }>("get_identity");
      keys.value = result.keys;
      displayName.value = result.displayName;
      const auth = useAuthStore();
      if (auth.profile) {
        const ws = useWebSocket();
        ws.connect(auth.profile.userId);
      }
    } catch {
      // no identity stored yet — first run
    }
  }

  async function createIdentity(name: string) {
    const auth = useAuthStore();

    // 1. Generate Ed25519 identity key + X25519 signed prekey locally
    const result = await invoke<{ keys: IdentityKeys }>("create_identity", { displayName: name });
    keys.value = result.keys;
    displayName.value = name;

    // 2. Fetch the full prekey bundle (public halves only) to send to the server
    const bundle = await invoke<{
      registrationId: number;
      identityKey: string;
      signedPreKey: { keyId: number; publicKey: string; signature: string };
      oneTimePreKey?: { keyId: number; publicKey: string };
    }>("generate_prekey_bundle");

    // 3. Register with the relay server — user_id is taken from the JWT server-side,
    //    so the client cannot forge it
    const api = useServerApi();
    await api.register({
      displayName: name,
      identityKey: bundle.identityKey,
      signedPreKey: bundle.signedPreKey,
      oneTimePreKeys: bundle.oneTimePreKey ? [bundle.oneTimePreKey] : [],
      registrationId: bundle.registrationId,
    });

    // 4. Connect WebSocket for real-time message delivery
    if (auth.profile) {
      const ws = useWebSocket();
      ws.connect(auth.profile.userId);
    }
  }

  return { keys, displayName, isSetup, initialize, createIdentity };
});
