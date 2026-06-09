<script setup lang="ts">
// @faridguzman: Settings view — profile, identity keys, invite generation, sign out.
import { ref } from "vue";
import { useRouter } from "vue-router";
import { useIdentityStore } from "../stores/identity";
import { useAuthStore } from "../stores/auth";
import { useServerApi } from "../composables/useServerApi";
import { open as shellOpen } from "@tauri-apps/plugin-opener";
import QRCode from "qrcode";
import Panel from "primevue/panel";
import Button from "primevue/button";
import Tag from "primevue/tag";
import Divider from "primevue/divider";
import InputText from "primevue/inputtext";

const router   = useRouter();
const identity = useIdentityStore();
const auth     = useAuthStore();
const api      = useServerApi();

// ── Invite state ──────────────────────────────────────────────────────────────
const inviteUrl    = ref("");
const inviteQr     = ref("");   // data URL rendered by qrcode
const inviteExpiry = ref<number | null>(null);
const inviteLoading = ref(false);
const copied       = ref(false);

async function generateInvite() {
  inviteLoading.value = true;
  try {
    const result = await api.createInvite();
    inviteUrl.value    = result.url;
    inviteExpiry.value = result.expiresAt;
    // @faridguzman: Render QR with brand colours — dark chip on dark background
    inviteQr.value = await QRCode.toDataURL(result.url, {
      width: 220,
      margin: 2,
      color: { dark: "#e8eaf6", light: "#1e1e2e" },
    });
  } catch {
    inviteUrl.value = "";
  } finally {
    inviteLoading.value = false;
  }
}

async function copyLink() {
  if (!inviteUrl.value) return;
  await navigator.clipboard.writeText(inviteUrl.value);
  copied.value = true;
  setTimeout(() => { copied.value = false; }, 2000);
}

async function shareEmail() {
  const body = encodeURIComponent(
    `I'd like to connect with you on engage — an end-to-end encrypted chat app.\n\nUse this link to add me as a contact:\n${inviteUrl.value}`
  );
  await shellOpen(`mailto:?subject=Join%20me%20on%20engage&body=${body}`).catch(() => {});
}

async function shareSms() {
  const body = encodeURIComponent(`Join me on engage: ${inviteUrl.value}`);
  await shellOpen(`sms:?body=${body}`).catch(() => {});
}

function formatExpiry(ms: number): string {
  const remaining = Math.max(0, ms - Date.now());
  const h = Math.floor(remaining / 3_600_000);
  return h > 0 ? `expires in ${h}h` : "expiring soon";
}

// ── Auth ──────────────────────────────────────────────────────────────────────
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

      <!-- @faridguzman: Invite panel — generate a short-lived link, show QR, copy, or share -->
      <Panel header="Invite someone">
        <div class="invite-section">
          <p class="invite-desc">
            Generate a 24-hour link. Anyone who opens it can add you as an encrypted contact.
          </p>

          <Button
            :label="inviteUrl ? 'Regenerate link' : 'Generate invite link'"
            icon="pi pi-user-plus"
            :loading="inviteLoading"
            style="background: var(--engage-accent); border-color: var(--engage-accent);"
            @click="generateInvite"
          />

          <template v-if="inviteUrl">
            <!-- Link field + copy -->
            <div class="invite-link-row">
              <InputText
                :model-value="inviteUrl"
                readonly
                class="invite-link-input"
                style="font-size:0.78rem;"
              />
              <Button
                :icon="copied ? 'pi pi-check' : 'pi pi-copy'"
                :severity="copied ? 'success' : 'secondary'"
                outlined
                size="small"
                v-tooltip.top="copied ? 'Copied!' : 'Copy link'"
                @click="copyLink"
              />
            </div>

            <p v-if="inviteExpiry" class="invite-expiry">
              <i class="pi pi-clock" style="font-size:0.75rem;" />
              {{ formatExpiry(inviteExpiry) }}
            </p>

            <!-- QR code -->
            <div class="qr-wrap">
              <img :src="inviteQr" alt="Invite QR code" class="qr-img" />
              <p class="qr-hint">Scan to add contact</p>
            </div>

            <!-- Share buttons -->
            <div class="share-row">
              <Button
                label="Email"
                icon="pi pi-envelope"
                size="small"
                severity="secondary"
                outlined
                @click="shareEmail"
              />
              <Button
                label="SMS"
                icon="pi pi-comment"
                size="small"
                severity="secondary"
                outlined
                @click="shareSms"
              />
            </div>
          </template>
        </div>
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

/* @faridguzman: Invite panel */
.invite-section {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}
.invite-desc {
  font-size: 0.83rem;
  color: var(--engage-muted);
  margin: 0;
  line-height: 1.5;
}
.invite-link-row {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}
.invite-link-input {
  flex: 1;
  min-width: 0;
  background: var(--engage-input-bg) !important;
}
.invite-expiry {
  font-size: 0.75rem;
  color: var(--engage-muted);
  margin: 0;
  display: flex;
  align-items: center;
  gap: 0.3rem;
}
.qr-wrap {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.4rem;
  padding: 0.75rem;
  background: var(--engage-input-bg);
  border-radius: 10px;
  align-self: center;
}
.qr-img {
  width: 160px;
  height: 160px;
  image-rendering: pixelated;
  border-radius: 6px;
}
.qr-hint {
  font-size: 0.72rem;
  color: var(--engage-muted);
  margin: 0;
}
.share-row {
  display: flex;
  gap: 0.6rem;
}
</style>
