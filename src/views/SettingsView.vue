<script setup lang="ts">
import { useRouter } from "vue-router";
import { useIdentityStore } from "../stores/identity";
import { useAuthStore } from "../stores/auth";
import Panel from "primevue/panel";
import Button from "primevue/button";
import Tag from "primevue/tag";
import Divider from "primevue/divider";

const router = useRouter();
const identity = useIdentityStore();
const auth = useAuthStore();

function logout() {
  auth.clearToken();
  router.push("/login");
}
</script>

<template>
  <div class="settings-wrap">
    <div class="settings-inner">
      <div class="settings-top">
        <Button icon="pi pi-arrow-left" text rounded @click="router.back()" />
        <h2>Settings</h2>
      </div>

      <Panel header="Your Profile">
        <div class="profile-row">
          <div class="big-avatar">{{ identity.displayName?.[0]?.toUpperCase() }}</div>
          <div>
            <p class="profile-name">{{ identity.displayName }}</p>
            <p class="profile-email">{{ auth.profile?.email }}</p>
          </div>
        </div>
      </Panel>

      <Panel header="Identity Keys" toggleable :collapsed="true">
        <div class="key-section">
          <p class="key-label">Identity public key (IK)</p>
          <code class="key-value">{{ identity.keys?.identityPublicKey }}</code>
          <Divider />
          <p class="key-label">Signed prekey (SPK)</p>
          <code class="key-value">{{ identity.keys?.signedPreKeyPublicKey }}</code>
        </div>
        <Tag
          value="End-to-end encrypted"
          icon="pi pi-lock"
          severity="success"
          class="mt-2"
        />
      </Panel>

      <Panel header="Account">
        <Button
          label="Sign out"
          icon="pi pi-sign-out"
          severity="danger"
          outlined
          class="w-full"
          @click="logout"
        />
      </Panel>
    </div>
  </div>
</template>

<style scoped>
.settings-wrap {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--engage-main-bg);
  overflow-y: auto;
}
.settings-inner {
  max-width: 600px;
  width: 100%;
  margin: 0 auto;
  padding: 1.5rem 1rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
.settings-top {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}
.settings-top h2 { margin: 0; font-size: 1.2rem; }
.profile-row { display: flex; align-items: center; gap: 1rem; }
.big-avatar {
  width: 56px; height: 56px; border-radius: 50%;
  background: var(--engage-accent);
  color: #fff; font-size: 1.5rem; font-weight: 700;
  display: flex; align-items: center; justify-content: center;
  flex-shrink: 0;
}
.profile-name { font-weight: 600; margin: 0; }
.profile-email { color: var(--engage-muted); font-size: 0.85rem; margin: 0; }
.key-section { display: flex; flex-direction: column; gap: 0.4rem; }
.key-label { font-size: 0.78rem; color: var(--engage-muted); margin: 0; text-transform: uppercase; letter-spacing: 0.04em; }
.key-value {
  font-size: 0.72rem; word-break: break-all;
  background: var(--engage-input-bg); padding: 0.5rem 0.75rem;
  border-radius: 6px; color: var(--engage-muted);
  display: block; line-height: 1.5;
}
.w-full { width: 100%; }
.mt-2 { margin-top: 0.75rem; }
</style>
