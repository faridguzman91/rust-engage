<script setup lang="ts">
import { useIdentityStore } from "../stores/identity";

const identity = useIdentityStore();
</script>

<template>
  <div class="settings-view">
    <h2>Settings</h2>

    <section>
      <h3>Your Identity</h3>
      <p><strong>Display name:</strong> {{ identity.displayName }}</p>
      <p class="key-label">Identity public key</p>
      <code class="key-value">{{ identity.keys?.identityPublicKey }}</code>
    </section>
  </div>
</template>

<style scoped>
.settings-view {
  padding: 2rem;
  max-width: 600px;
}
h2 { margin-bottom: 1.5rem; }
h3 { color: var(--text-muted); font-size: 0.85rem; text-transform: uppercase; margin-bottom: 0.5rem; }
section { margin-bottom: 2rem; }
.key-label { font-size: 0.8rem; color: var(--text-muted); margin-bottom: 0.25rem; }
.key-value {
  display: block;
  font-size: 0.75rem;
  word-break: break-all;
  background: var(--bg-secondary);
  padding: 0.5rem;
  border-radius: 6px;
  color: var(--text-muted);
}
</style>
