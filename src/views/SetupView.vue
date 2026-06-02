<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { useIdentityStore } from "../stores/identity";

const router = useRouter();
const identity = useIdentityStore();

const displayName = ref("");
const loading = ref(false);
const error = ref("");

async function setup() {
  if (!displayName.value.trim()) return;
  loading.value = true;
  error.value = "";
  try {
    await identity.createIdentity(displayName.value.trim());
    router.push("/chat");
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="setup-view">
    <div class="setup-card">
      <h1 class="logo">engage</h1>
      <p class="subtitle">End-to-end encrypted messaging</p>

      <form @submit.prevent="setup">
        <label for="display-name">Your display name</label>
        <input
          id="display-name"
          v-model="displayName"
          type="text"
          placeholder="e.g. Alice"
          autocomplete="off"
          required
        />
        <p v-if="error" class="error">{{ error }}</p>
        <button type="submit" :disabled="loading || !displayName.trim()">
          {{ loading ? "Generating keys…" : "Create identity" }}
        </button>
      </form>

      <p class="hint">
        Your identity keys are generated locally and never leave your device unencrypted.
      </p>
    </div>
  </div>
</template>

<style scoped>
.setup-view {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  background: var(--bg-primary);
}
.setup-card {
  width: 360px;
  padding: 2.5rem 2rem;
  background: var(--bg-secondary);
  border-radius: 12px;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.15);
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
.logo {
  font-size: 2rem;
  font-weight: 700;
  color: var(--accent);
  text-align: center;
  margin: 0;
}
.subtitle {
  text-align: center;
  color: var(--text-muted);
  margin: 0;
  font-size: 0.9rem;
}
form {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}
label {
  font-size: 0.85rem;
  color: var(--text-muted);
}
input {
  padding: 0.6rem 0.8rem;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-input);
  color: var(--text-primary);
  font-size: 1rem;
}
button {
  padding: 0.7rem;
  border-radius: 8px;
  border: none;
  background: var(--accent);
  color: #fff;
  font-size: 1rem;
  cursor: pointer;
  transition: opacity 0.15s;
}
button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.error {
  color: var(--danger);
  font-size: 0.85rem;
}
.hint {
  font-size: 0.75rem;
  color: var(--text-muted);
  text-align: center;
}
</style>
