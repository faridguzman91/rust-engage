import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useServerApi } from "../composables/useServerApi";
import { useWebSocket } from "../composables/useWebSocket";
import { useAuthStore } from "./auth";

export interface IdentityKeys {
  identityPublicKey: string;
  signedPreKeyPublicKey: string;
  registrationId: number;
}

export const useIdentityStore = defineStore("identity", () => {
  const keys = ref<IdentityKeys | null>(null);
  const displayName = ref<string>("");

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
      // no identity yet
    }
  }

  async function createIdentity(name: string) {
    const auth = useAuthStore();

    // 1. Generate keys locally
    const result = await invoke<{ keys: IdentityKeys }>("create_identity", { displayName: name });
    keys.value = result.keys;
    displayName.value = name;

    // 2. Fetch our full prekey bundle
    const bundle = await invoke<{
      registrationId: number;
      identityKey: string;
      signedPreKey: { keyId: number; publicKey: string; signature: string };
      oneTimePreKey?: { keyId: number; publicKey: string };
    }>("generate_prekey_bundle");

    // 3. Register with relay server (user_id comes from JWT on server side)
    const api = useServerApi();
    await api.register({
      displayName: name,
      identityKey: bundle.identityKey,
      signedPreKey: bundle.signedPreKey,
      oneTimePreKeys: bundle.oneTimePreKey ? [bundle.oneTimePreKey] : [],
      registrationId: bundle.registrationId,
    });

    // 4. Connect WebSocket
    if (auth.profile) {
      const ws = useWebSocket();
      ws.connect(auth.profile.userId);
    }
  }

  return { keys, displayName, isSetup, initialize, createIdentity };
});
