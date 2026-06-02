import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";

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
    } catch {
      // no identity yet
    }
  }

  async function createIdentity(name: string) {
    const result = await invoke<{ keys: IdentityKeys }>("create_identity", { displayName: name });
    keys.value = result.keys;
    displayName.value = name;
  }

  return { keys, displayName, isSetup, initialize, createIdentity };
});
