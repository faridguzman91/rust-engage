<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91> -->
<script setup lang="ts">
// @faridguzman: Login screen — supports Google OAuth and optional Nextcloud OAuth.
// On mount, fetches /api/auth/providers to discover which providers the server
// has configured.  The Nextcloud button only renders when NEXTCLOUD_URL etc.
// are set in the server's environment.
import { ref, onMounted } from "vue";
import { useAuthStore } from "../stores/auth";
import { useServerApi } from "../composables/useServerApi";
import Card from "primevue/card";
import Button from "primevue/button";
import Divider from "primevue/divider";

const auth = useAuthStore();
const api  = useServerApi();

const googleLoading    = ref(false);
const nextcloudLoading = ref(false);

const nextcloudEnabled    = ref(false);
const nextcloudServerName = ref("Nextcloud");
const nextcloudServerUrl  = ref("");

onMounted(async () => {
  try {
    const providers = await api.fetchAuthProviders();
    nextcloudEnabled.value    = providers.nextcloud.enabled;
    nextcloudServerName.value = providers.nextcloud.serverName || "Nextcloud";
    nextcloudServerUrl.value  = providers.nextcloud.serverUrl;
  } catch {
    // Server unreachable or providers endpoint not supported — show Google only
  }
});

function loginWithGoogle() {
  googleLoading.value = true;
  auth.loginWithGoogle();
}

function loginWithNextcloud() {
  nextcloudLoading.value = true;
  auth.loginWithNextcloud();
}
</script>

<template>
  <div class="login-wrap">
    <Card class="login-card">
      <template #content>
        <div class="login-body">
          <img src="/engage.svg" alt="engage" class="logo" />

          <div class="text-center">
            <p class="tagline">End-to-end encrypted messaging</p>
          </div>

          <!-- Google OAuth -->
          <Button
            class="provider-btn"
            :loading="googleLoading"
            :label="googleLoading ? 'Opening browser…' : 'Continue with Google'"
            severity="secondary"
            outlined
            @click="loginWithGoogle"
          >
            <template #icon>
              <svg viewBox="0 0 24 24" width="18" height="18" xmlns="http://www.w3.org/2000/svg" style="flex-shrink:0">
                <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" fill="#4285F4"/>
                <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853"/>
                <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l3.66-2.84z" fill="#FBBC05"/>
                <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335"/>
              </svg>
            </template>
          </Button>

          <!-- Nextcloud OAuth — only shown when server has it configured -->
          <template v-if="nextcloudEnabled">
            <Divider>
              <span style="font-size:0.75rem; color: var(--engage-muted);">or</span>
            </Divider>

            <Button
              class="provider-btn nextcloud-btn"
              :loading="nextcloudLoading"
              :label="nextcloudLoading ? 'Opening browser…' : `Sign in with ${nextcloudServerName}`"
              severity="secondary"
              outlined
              @click="loginWithNextcloud"
            >
              <template #icon>
                <!-- @faridguzman: Nextcloud logo mark (simplified cloud icon) -->
                <svg viewBox="0 0 48 48" width="18" height="18" xmlns="http://www.w3.org/2000/svg" style="flex-shrink:0">
                  <path d="M24 8C14.06 8 6 16.06 6 26s8.06 18 18 18 18-8.06 18-18S33.94 8 24 8zm0 4a14 14 0 0 1 14 14 14 14 0 0 1-14 14A14 14 0 0 1 10 26 14 14 0 0 1 24 12z" fill="#0082C9"/>
                  <circle cx="17" cy="26" r="5" fill="#0082C9"/>
                  <circle cx="24" cy="22" r="5" fill="#0082C9"/>
                  <circle cx="31" cy="26" r="5" fill="#0082C9"/>
                </svg>
              </template>
            </Button>

            <p v-if="nextcloudServerUrl" class="nc-url-hint">
              <i class="pi pi-server" style="font-size:0.7rem;" />
              {{ nextcloudServerUrl }}
            </p>
          </template>

          <p class="hint">
            Your messages are encrypted on your device.<br />
            Your identity provider is only used to verify who you are.
          </p>
        </div>
      </template>
    </Card>
  </div>
</template>

<style scoped>
.login-wrap {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  background: var(--engage-main-bg);
}
.login-card {
  width: 380px;
  background: var(--engage-sidebar-bg) !important;
  border: 1px solid var(--engage-border) !important;
  border-radius: 16px !important;
}
.login-body {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.25rem;
  padding: 0.5rem 0;
}
.logo { width: 240px; }
.tagline { color: var(--engage-muted); font-size: 0.9rem; margin: 0; }

.provider-btn {
  width: 100%;
  justify-content: center;
  gap: 0.75rem;
}

/* @faridguzman: Nextcloud brand tint on hover */
.nextcloud-btn:hover {
  border-color: #0082C9 !important;
  color: #0082C9 !important;
}

.nc-url-hint {
  font-size: 0.72rem;
  color: var(--engage-muted);
  margin: -0.5rem 0 0;
  display: flex;
  align-items: center;
  gap: 0.3rem;
}

.hint {
  font-size: 0.75rem;
  color: var(--engage-muted);
  text-align: center;
  line-height: 1.6;
}
</style>
