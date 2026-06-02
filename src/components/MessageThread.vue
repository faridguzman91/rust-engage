<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from "vue";
import { useMessagesStore } from "../stores/messages";
import { useContactsStore } from "../stores/contacts";
import { useIdentityStore } from "../stores/identity";

const props = defineProps<{ contactId: string }>();

const messages = useMessagesStore();
const contacts = useContactsStore();
const identity = useIdentityStore();

const input = ref("");
const threadEl = ref<HTMLElement | null>(null);
const sending = ref(false);

const contact = computed(() => contacts.getById(props.contactId));
const msgs = computed(() => messages.forConversation(props.contactId));

async function load() {
  await messages.load(props.contactId);
  scrollToBottom();
}

async function send() {
  if (!input.value.trim() || sending.value) return;
  const body = input.value.trim();
  input.value = "";
  sending.value = true;
  try {
    await messages.send(props.contactId, body);
    scrollToBottom();
  } finally {
    sending.value = false;
  }
}

function scrollToBottom() {
  nextTick(() => {
    if (threadEl.value) {
      threadEl.value.scrollTop = threadEl.value.scrollHeight;
    }
  });
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

onMounted(load);
watch(() => props.contactId, load);
</script>

<template>
  <div class="message-thread">
    <header class="thread-header">
      <div v-if="contact" class="contact-info">
        <div class="avatar">{{ contact.displayName[0]?.toUpperCase() }}</div>
        <span class="contact-name">{{ contact.displayName }}</span>
      </div>
      <span class="e2e-badge">🔒 End-to-end encrypted</span>
    </header>

    <div ref="threadEl" class="messages">
      <div
        v-for="msg in msgs"
        :key="msg.id"
        class="message"
        :class="{ mine: msg.isMine, theirs: !msg.isMine }"
      >
        <div class="bubble">
          <span class="body">{{ msg.body }}</span>
          <span class="meta">{{ formatTime(msg.timestamp) }} <span class="status">{{ msg.status === 'read' ? '✓✓' : '✓' }}</span></span>
        </div>
      </div>
    </div>

    <form class="composer" @submit.prevent="send">
      <input
        v-model="input"
        placeholder="Message"
        autocomplete="off"
        @keydown.enter.exact.prevent="send"
      />
      <button type="submit" :disabled="!input.trim() || sending">
        Send
      </button>
    </form>
  </div>
</template>

<style scoped>
.message-thread {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.thread-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid var(--border);
  background: var(--bg-secondary);
}
.contact-info { display: flex; align-items: center; gap: 0.6rem; }
.avatar {
  width: 32px; height: 32px; border-radius: 50%;
  background: var(--accent); color: #fff;
  display: flex; align-items: center; justify-content: center;
  font-weight: 600; font-size: 0.9rem;
}
.contact-name { font-weight: 600; }
.e2e-badge { font-size: 0.75rem; color: var(--text-muted); }
.messages {
  flex: 1;
  overflow-y: auto;
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.message { display: flex; }
.message.mine { justify-content: flex-end; }
.message.theirs { justify-content: flex-start; }
.bubble {
  max-width: 65%;
  padding: 0.5rem 0.75rem;
  border-radius: 12px;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}
.message.mine .bubble {
  background: var(--accent);
  color: #fff;
  border-bottom-right-radius: 4px;
}
.message.theirs .bubble {
  background: var(--bg-secondary);
  color: var(--text-primary);
  border-bottom-left-radius: 4px;
}
.body { font-size: 0.95rem; }
.meta { font-size: 0.7rem; opacity: 0.7; align-self: flex-end; }
.composer {
  display: flex;
  gap: 0.5rem;
  padding: 0.75rem 1rem;
  border-top: 1px solid var(--border);
  background: var(--bg-secondary);
}
.composer input {
  flex: 1;
  padding: 0.5rem 0.75rem;
  border-radius: 20px;
  border: 1px solid var(--border);
  background: var(--bg-input);
  color: var(--text-primary);
  font-size: 0.95rem;
}
.composer button {
  padding: 0.5rem 1rem;
  border-radius: 20px;
  border: none;
  background: var(--accent);
  color: #fff;
  font-size: 0.9rem;
  cursor: pointer;
}
.composer button:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
