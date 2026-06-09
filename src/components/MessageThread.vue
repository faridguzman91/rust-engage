<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91> -->
<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useMessagesStore } from "../stores/messages";
import { useContactsStore } from "../stores/contacts";
import { useDisappearingMessages, TIMER_OPTIONS, formatTimer } from "../composables/useDisappearingMessages";
import { useWebSocket } from "../composables/useWebSocket";
import { useWebRTC } from "../composables/useWebRTC";
import Avatar from "primevue/avatar";
import Button from "primevue/button";
import InputText from "primevue/inputtext";
import Tag from "primevue/tag";
import ProgressSpinner from "primevue/progressspinner";
import Select from "primevue/select";

const props = defineProps<{ contactId: string }>();

const messages = useMessagesStore();
const contacts = useContactsStore();
const { scheduleExpiry, startSweep } = useDisappearingMessages();
const ws  = useWebSocket();
const rtc = useWebRTC();

const input = ref("");
const threadEl = ref<HTMLElement | null>(null);
const sending = ref(false);
const loading = ref(false);
const timerSecs = ref(0);
let sweepInterval: ReturnType<typeof setInterval> | null = null;

const contact = computed(() => contacts.getById(props.contactId));
const msgs = computed(() => messages.forConversation(props.contactId));

// @faridguzman: Format the expiry countdown for display on a bubble
function formatCountdown(expiresAt: number): string {
  const remaining = Math.max(0, Math.ceil((expiresAt - Date.now()) / 1000));
  if (remaining < 60)   return `${remaining}s`;
  if (remaining < 3600) return `${Math.ceil(remaining / 60)}m`;
  if (remaining < 86400)return `${Math.ceil(remaining / 3600)}h`;
  return `${Math.ceil(remaining / 86400)}d`;
}

async function load() {
  loading.value = true;
  await messages.load(props.contactId);
  loading.value = false;
  scrollToBottom();

  // Load the current timer setting for this conversation
  timerSecs.value = await invoke<number>("get_disappear_timer", { contactId: props.contactId });

  // Schedule JS timers for any message with a future expiry
  await startSweep(msgs.value);

  // @faridguzman: Emit read receipts for all received messages now that
  // the user has the conversation open.
  ws.markRead(props.contactId);
}

async function send() {
  if (!input.value.trim() || sending.value) return;
  const body = input.value.trim();
  input.value = "";
  sending.value = true;
  try {
    const msg = await messages.send(props.contactId, body);
    scheduleExpiry(msg);
    scrollToBottom();
  } finally {
    sending.value = false;
  }
}

async function onTimerChange(secs: number) {
  timerSecs.value = secs;
  await invoke("set_disappear_timer", { contactId: props.contactId, secs });
}

function scrollToBottom() {
  nextTick(() => {
    if (threadEl.value) threadEl.value.scrollTop = threadEl.value.scrollHeight;
  });
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function avatarLabel(name?: string) {
  return name?.[0]?.toUpperCase() ?? "?";
}

onMounted(() => {
  load();
  // @faridguzman: Periodic sweep every 30s to clean up expired messages
  sweepInterval = setInterval(async () => {
    await invoke("sweep_expired_messages");
  }, 30_000);
});

onUnmounted(() => {
  if (sweepInterval) clearInterval(sweepInterval);
});

watch(() => props.contactId, load);

// @faridguzman: When a new message arrives while this thread is visible,
// mark it read immediately (the WS handler already appended it to the store).
watch(msgs, (next, prev) => {
  if (next.length > (prev?.length ?? 0)) {
    ws.markRead(props.contactId);
    scrollToBottom();
  }
});
</script>

<template>
  <div class="thread">
    <!-- Header -->
    <div class="thread-header">
      <div class="header-left">
        <Avatar
          :label="avatarLabel(contact?.displayName)"
          shape="circle"
          size="normal"
          style="background: #4a4a78; color: #e8eaf6; font-weight:600; flex-shrink:0;"
        />
        <div>
          <p class="contact-name">{{ contact?.displayName }}</p>
          <Tag value="End-to-end encrypted" icon="pi pi-lock" severity="success" style="font-size:0.68rem; padding: 0.15rem 0.5rem;" />
        </div>
      </div>
      <div class="header-actions">
        <!-- @faridguzman: Disappear timer picker -->
        <div class="timer-wrap" v-tooltip.bottom="'Disappearing messages'">
          <i class="pi pi-clock timer-icon" :class="{ active: timerSecs > 0 }" />
          <Select
            :model-value="timerSecs"
            :options="TIMER_OPTIONS"
            option-label="label"
            option-value="value"
            class="timer-select"
            @update:model-value="onTimerChange"
          />
        </div>
        <!-- @faridguzman: Voice and video call buttons — wired to useWebRTC -->
        <Button
          icon="pi pi-phone"
          text rounded size="small"
          v-tooltip.bottom="'Voice call'"
          :disabled="!rtc.status.value === undefined || rtc.status.value !== 'idle'"
          @click="rtc.startCall(props.contactId, false)"
        />
        <Button
          icon="pi pi-video"
          text rounded size="small"
          v-tooltip.bottom="'Video call'"
          :disabled="rtc.status.value !== 'idle'"
          @click="rtc.startCall(props.contactId, true)"
        />
        <Button icon="pi pi-ellipsis-v" text rounded size="small" v-tooltip.bottom="'More options'" />
      </div>
    </div>

    <!-- Disappear timer banner -->
    <div v-if="timerSecs > 0" class="timer-banner">
      <i class="pi pi-clock" />
      Messages disappear after <strong>{{ formatTimer(timerSecs) }}</strong>
    </div>

    <!-- Messages -->
    <div ref="threadEl" class="messages-area">
      <div v-if="loading" class="loading-state">
        <ProgressSpinner style="width:32px;height:32px;" strokeWidth="4" />
      </div>

      <template v-else>
        <div v-if="msgs.length === 0" class="empty-thread">
          <i class="pi pi-lock" style="font-size:2rem; opacity:0.2;" />
          <p>Messages are end-to-end encrypted.<br />Say hello!</p>
        </div>

        <div
          v-for="msg in msgs"
          :key="msg.id"
          class="msg-row"
          :class="msg.isMine ? 'mine' : 'theirs'"
        >
          <Avatar
            v-if="!msg.isMine"
            :label="avatarLabel(contact?.displayName)"
            shape="circle"
            size="small"
            style="background:#4a4a78;color:#e8eaf6;font-weight:600;flex-shrink:0;align-self:flex-end;"
          />
          <div class="bubble" :class="msg.isMine ? 'bubble-mine' : 'bubble-theirs'">
            <span class="bubble-body">{{ msg.body }}</span>
            <span class="bubble-meta">
              {{ formatTime(msg.timestamp) }}
              <i
                v-if="msg.isMine"
                class="pi"
                :class="msg.status === 'read' ? 'pi-check-circle' : 'pi-check'"
                style="font-size:0.65rem;"
              />
              <!-- @faridguzman: Expiry countdown shown on disappearing messages -->
              <span v-if="msg.expiresAt" class="expiry-badge">
                <i class="pi pi-clock" style="font-size:0.6rem;" />
                {{ formatCountdown(msg.expiresAt) }}
              </span>
            </span>
          </div>
        </div>
      </template>
    </div>

    <!-- Composer -->
    <div class="composer">
      <Button icon="pi pi-paperclip" text rounded size="small" v-tooltip.top="'Attach file (coming soon)'" disabled />
      <InputText
        v-model="input"
        placeholder="Message"
        class="composer-input"
        autocomplete="off"
        @keydown.enter.exact.prevent="send"
      />
      <Button icon="pi pi-face-smile" text rounded size="small" v-tooltip.top="'Emoji (coming soon)'" disabled />
      <Button
        icon="pi pi-send"
        rounded
        size="small"
        :disabled="!input.trim() || sending"
        :loading="sending"
        style="background: var(--engage-accent); border-color: var(--engage-accent);"
        @click="send"
      />
    </div>
  </div>
</template>

<style scoped>
.thread {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--engage-main-bg);
}
.thread-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1.25rem;
  border-bottom: 1px solid var(--engage-border);
  background: var(--engage-header-bg);
  flex-shrink: 0;
}
.header-left { display: flex; align-items: center; gap: 0.75rem; }
.contact-name { font-weight: 600; font-size: 0.95rem; margin: 0 0 0.2rem; }
.header-actions { display: flex; align-items: center; gap: 0.25rem; }

/* @faridguzman: Timer picker — clock icon + hidden select overlay */
.timer-wrap {
  display: flex;
  align-items: center;
  position: relative;
  cursor: pointer;
}
.timer-icon {
  font-size: 1rem;
  color: var(--engage-muted);
  transition: color 0.15s;
}
.timer-icon.active { color: var(--engage-accent); }
.timer-select {
  position: absolute;
  inset: 0;
  opacity: 0;
  width: 100%;
  cursor: pointer;
}

.timer-banner {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.35rem 1.25rem;
  background: rgba(62, 191, 140, 0.08);
  border-bottom: 1px solid rgba(62, 191, 140, 0.2);
  font-size: 0.78rem;
  color: var(--engage-accent);
  flex-shrink: 0;
}

.messages-area {
  flex: 1;
  overflow-y: auto;
  padding: 1.25rem 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}
.loading-state, .empty-thread {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  color: var(--engage-muted);
  font-size: 0.85rem;
  text-align: center;
  line-height: 1.6;
}

.msg-row {
  display: flex;
  align-items: flex-end;
  gap: 0.5rem;
  max-width: 72%;
}
.msg-row.mine { align-self: flex-end; flex-direction: row-reverse; }
.msg-row.theirs { align-self: flex-start; }

.bubble {
  padding: 0.55rem 0.9rem;
  border-radius: 18px;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  max-width: 100%;
  word-break: break-word;
}
.bubble-mine {
  background: var(--engage-sent-bg);
  color: var(--engage-sent-fg);
  border-bottom-right-radius: 4px;
}
.bubble-theirs {
  background: var(--engage-recv-bg);
  color: var(--engage-recv-fg);
  border-bottom-left-radius: 4px;
}
.bubble-body { font-size: 0.92rem; line-height: 1.45; }
.bubble-meta {
  font-size: 0.68rem;
  opacity: 0.65;
  align-self: flex-end;
  display: flex;
  align-items: center;
  gap: 0.25rem;
  white-space: nowrap;
}
.expiry-badge {
  display: flex;
  align-items: center;
  gap: 0.15rem;
  color: var(--engage-accent);
  opacity: 1;
}

.composer {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.75rem 1rem;
  border-top: 1px solid var(--engage-border);
  background: var(--engage-header-bg);
  flex-shrink: 0;
}
.composer-input {
  flex: 1;
  border-radius: 24px !important;
  background: var(--engage-input-bg) !important;
  border-color: transparent !important;
  font-size: 0.93rem;
}
.composer-input:focus {
  border-color: var(--engage-accent) !important;
  box-shadow: 0 0 0 1px var(--engage-accent) !important;
}
</style>
