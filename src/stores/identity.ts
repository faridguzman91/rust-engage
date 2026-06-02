import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useServerApi } from "../composables/useServerApi";
import { useWebSocket } from "../composables/useWebSocket";

export interface IdentityKeys {
  identityPublicKey: string;
  signedPreKeyPublicKey: string;
  registrationId: number;
}

export const useIdentityStore = defineStore("identity", () => {
  const keys = ref<IdentityKeys | null>(null);
  const displayName = ref<string>("");
  const userId = ref<string>("");

  const isSetup = computed(() => keys.value !== null && displayName.value !== "");

  async function initialize() {
    try {
      const result = await invoke<{ keys: IdentityKeys; displayName: string }>("get_identity");
      keys.value = result.keys;
      displayName.value = result.displayName;
      // Derive a stable user ID from the identity public key (first 32 chars)
      userId.value = result.keys.identityPublicKey.replace(/[^a-zA-Z0-9]/g, "").slice(0, 32);
      // Re-connect WebSocket on app restart
      const ws = useWebSocket();
      ws.connect(userId.value);
    } catch {
      // no identity yet
    }
  }

  async function createIdentity(name: string) {
    // 1. Generate keys locally
    const result = await invoke<{ keys: IdentityKeys }>("create_identity", { displayName: name });
    keys.value = result.keys;
    displayName.value = name;
    userId.value = result.keys.identityPublicKey.replace(/[^a-zA-Z0-9]/g, "").slice(0, 32);

    // 2. Fetch our full prekey bundle (includes signed prekey + one-time prekeys)
    const bundle = await invoke<{
      registrationId: number;
      identityKey: string;
      signedPreKey: { keyId: number; publicKey: string; signature: string };
      oneTimePreKey?: { keyId: number; publicKey: string };
    }>("generate_prekey_bundle");

    // 3. Register with relay server
    const api = useServerApi();
    await api.register({
      userId: userId.value,
      displayName: name,
      identityKey: bundle.identityKey,
      signedPreKey: bundle.signedPreKey,
      oneTimePreKeys: bundle.oneTimePreKey ? [bundle.oneTimePreKey] : [],
      registrationId: bundle.registrationId,
    });

    // 4. Connect WebSocket
    const ws = useWebSocket();
    ws.connect(userId.value);
  }

  return { keys, displayName, userId, isSetup, initialize, createIdentity };
});
