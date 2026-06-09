<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91> -->
<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { useIdentityStore } from "../stores/identity";
import Card from "primevue/card";
import InputText from "primevue/inputtext";
import Button from "primevue/button";
import Message from "primevue/message";
import FloatLabel from "primevue/floatlabel";

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
  <div class="setup-wrap">
    <Card class="setup-card">
      <template #header>
        <div class="setup-header">
          <img src="/engage.png" alt="engage" class="logo" />
          <p class="subtitle">Set up your identity</p>
        </div>
      </template>
      <template #content>
        <div class="setup-body">
          <FloatLabel variant="on">
            <InputText
              id="display-name"
              v-model="displayName"
              autocomplete="off"
              class="w-full"
              @keydown.enter="setup"
            />
            <label for="display-name">Your display name</label>
          </FloatLabel>

          <Message v-if="error" severity="error" :closable="false" class="w-full">{{ error }}</Message>

          <Button
            label="Create identity"
            icon="pi pi-key"
            class="w-full"
            :loading="loading"
            :disabled="!displayName.trim()"
            @click="setup"
          />

          <p class="hint">
            <i class="pi pi-lock" style="font-size:0.75rem" />
            Keys are generated locally and never leave your device unencrypted.
          </p>
        </div>
      </template>
    </Card>
  </div>
</template>

<style scoped>
.setup-wrap {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  background: var(--engage-main-bg);
}
.setup-card {
  width: 380px;
  background: var(--engage-sidebar-bg) !important;
  border: 1px solid var(--engage-border) !important;
  border-radius: 16px !important;
}
.setup-header {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.5rem;
  padding: 1.5rem 1.5rem 0;
}
.logo { width: 180px; }
.subtitle { color: var(--engage-muted); font-size: 0.9rem; margin: 0; }
.setup-body {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
.hint {
  font-size: 0.75rem;
  color: var(--engage-muted);
  text-align: center;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.35rem;
}
.w-full { width: 100%; }
</style>
