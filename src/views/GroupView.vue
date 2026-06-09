<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91> -->
<script setup lang="ts">
// @faridguzman91: Group conversation thread — mirrors MessageThread but uses
// Sender Key encryption and shows member avatars in the header.
import { ref, computed, onMounted, watch, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useGroupsStore } from "../stores/groups";
import { useMessagesStore } from "../stores/messages";
import { useIdentityStore } from "../stores/identity";
import { useServerApi } from "../composables/useServerApi";
import Avatar from "primevue/avatar";
import AvatarGroup from "primevue/avatargroup";
import Button from "primevue/button";
import InputText from "primevue/inputtext";
import Tag from "primevue/tag";
import ProgressSpinner from "primevue/progressspinner";

const props = defineProps<{ groupId: string }>();

const groups = useGroupsStore();
const messages = useMessagesStore();
const identity = useIdentityStore();
const api = useServerApi();

const input = ref("");
const threadEl = ref<HTMLElement | null>(null);
const sending = ref(false);
const loading = ref(false);

const group = computed(() => groups.getById(props.groupId));
const msgs = computed(() => messages.forConversation(props.groupId));

async function load() {
  loading.value = true;
  // Group messages are stored in the same messages table keyed by groupId
  await messages.load(props.groupId);
  loading.value = false;
  scrollToBottom();
}

async function send() {
  if (!input.value.trim() || sending.value) return;
  const body = input.value.trim();
  input.value = "";
  sending.value = true;
  try {
    // @faridguzman: Sender Key encryption — one encrypt for all members
    const ciphertext = await groups.encryptMessage(props.groupId, body);
    await api.sendGroupEnvelope(props.groupId, {
      senderIk: identity.keys?.identityPublicKey ?? "",
      ciphertext,
    });
    // Persist locally as plaintext (we already know what we sent)
    const msg = await invoke<import("../stores/messages").Message>("send_message", {
      conversationId: props.groupId,
      body,
    });
    messages.append({ ...msg, conversationId: props.groupId });
    scrollToBottom();
  } finally {
    sending.value = false;
  }
}

function scrollToBottom() {
  nextTick(() => {
    if (threadEl.value) threadEl.value.scrollTop = threadEl.value.scrollHeight;
  });
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function senderName(senderId: string): string {
  return group.value?.members.find((m) => m.userId === senderId)?.displayName ?? senderId;
}

onMounted(load);
watch(() => props.groupId, load);
</script>

<template>
  <div class="group-thread">
    <!-- Header -->
    <div class="thread-header">
      <div class="header-left">
        <div class="group-icon">
          <i class="pi pi-users" />
        </div>
        <div>
          <p class="group-name">{{ group?.name }}</p>
          <div class="member-row">
            <AvatarGroup>
              <Avatar
                v-for="m in group?.members.slice(0, 5)"
                :key="m.userId"
                :label="m.displayName[0]?.toUpperCase()"
                shape="circle"
                size="small"
                style="background:#4a4a78;color:#e8eaf6;font-size:0.65rem;"
              />
            </AvatarGroup>
            <span class="member-count">{{ group?.members.length }} members</span>
          </div>
        </div>
        <Tag value="End-to-end encrypted" icon="pi pi-lock" severity="success" style="font-size:0.68rem; padding:0.15rem 0.5rem; margin-left:0.5rem;" />
      </div>
      <div class="header-actions">
        <Button icon="pi pi-user-plus" text rounded size="small" v-tooltip.bottom="'Add member (coming soon)'" disabled />
        <Button icon="pi pi-ellipsis-v" text rounded size="small" v-tooltip.bottom="'Group settings'" />
      </div>
    </div>

    <!-- Messages -->
    <div ref="threadEl" class="messages-area">
      <div v-if="loading" class="loading-state">
        <ProgressSpinner style="width:32px;height:32px;" strokeWidth="4" />
      </div>
      <template v-else>
        <div v-if="msgs.length === 0" class="empty-thread">
          <i class="pi pi-users" style="font-size:2rem;opacity:0.2;" />
          <p>Group messages are end-to-end encrypted.<br />Say hello!</p>
        </div>
        <div
          v-for="msg in msgs"
          :key="msg.id"
          class="msg-row"
          :class="msg.isMine ? 'mine' : 'theirs'"
        >
          <div v-if="!msg.isMine" class="avatar-col">
            <Avatar
              :label="senderName(msg.senderId)[0]?.toUpperCase()"
              shape="circle"
              size="small"
              style="background:#4a4a78;color:#e8eaf6;font-weight:600;flex-shrink:0;"
            />
          </div>
          <div class="bubble" :class="msg.isMine ? 'bubble-mine' : 'bubble-theirs'">
            <!-- @faridguzman91: Show sender name in group threads -->
            <span v-if="!msg.isMine" class="sender-name">{{ senderName(msg.senderId) }}</span>
            <span class="bubble-body">{{ msg.body }}</span>
            <span class="bubble-meta">{{ formatTime(msg.timestamp) }}</span>
          </div>
        </div>
      </template>
    </div>

    <!-- Composer -->
    <div class="composer">
      <InputText
        v-model="input"
        placeholder="Message group"
        class="composer-input"
        autocomplete="off"
        @keydown.enter.exact.prevent="send"
      />
      <Button
        icon="pi pi-send"
        rounded
        size="small"
        :disabled="!input.trim() || sending"
        :loading="sending"
        style="background:var(--engage-accent);border-color:var(--engage-accent);"
        @click="send"
      />
    </div>
  </div>
</template>

<style scoped>
.group-thread { display:flex; flex-direction:column; height:100%; background:var(--engage-main-bg); }
.thread-header {
  display:flex; align-items:center; justify-content:space-between;
  padding:0.75rem 1.25rem; border-bottom:1px solid var(--engage-border);
  background:var(--engage-header-bg); flex-shrink:0;
}
.header-left { display:flex; align-items:center; gap:0.75rem; }
.group-icon {
  width:40px; height:40px; border-radius:50%;
  background:var(--engage-accent); color:#fff;
  display:flex; align-items:center; justify-content:center;
  font-size:1.1rem; flex-shrink:0;
}
.group-name { font-weight:600; font-size:0.95rem; margin:0 0 0.2rem; }
.member-row { display:flex; align-items:center; gap:0.4rem; }
.member-count { font-size:0.72rem; color:var(--engage-muted); }
.header-actions { display:flex; gap:0.25rem; }

.messages-area {
  flex:1; overflow-y:auto; padding:1.25rem 1.5rem;
  display:flex; flex-direction:column; gap:0.4rem;
}
.loading-state, .empty-thread {
  flex:1; display:flex; flex-direction:column;
  align-items:center; justify-content:center; gap:0.75rem;
  color:var(--engage-muted); font-size:0.85rem; text-align:center; line-height:1.6;
}
.msg-row { display:flex; align-items:flex-end; gap:0.5rem; max-width:72%; }
.msg-row.mine { align-self:flex-end; flex-direction:row-reverse; }
.msg-row.theirs { align-self:flex-start; }
.avatar-col { flex-shrink:0; align-self:flex-end; }
.bubble {
  padding:0.55rem 0.9rem; border-radius:18px;
  display:flex; flex-direction:column; gap:0.15rem;
  max-width:100%; word-break:break-word;
}
.bubble-mine { background:var(--engage-sent-bg); color:var(--engage-sent-fg); border-bottom-right-radius:4px; }
.bubble-theirs { background:var(--engage-recv-bg); color:var(--engage-recv-fg); border-bottom-left-radius:4px; }
.sender-name { font-size:0.72rem; font-weight:600; color:var(--engage-accent); margin-bottom:0.1rem; }
.bubble-body { font-size:0.92rem; line-height:1.45; }
.bubble-meta { font-size:0.68rem; opacity:0.65; align-self:flex-end; }

.composer {
  display:flex; align-items:center; gap:0.5rem;
  padding:0.75rem 1rem; border-top:1px solid var(--engage-border);
  background:var(--engage-header-bg); flex-shrink:0;
}
.composer-input {
  flex:1; border-radius:24px !important;
  background:var(--engage-input-bg) !important; border-color:transparent !important; font-size:0.93rem;
}
</style>
