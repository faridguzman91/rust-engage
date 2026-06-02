<script setup lang="ts">
import { onMounted } from "vue";
import { useContactsStore } from "../stores/contacts";
import ConversationList from "../components/ConversationList.vue";
import MessageThread from "../components/MessageThread.vue";
import { useRoute } from "vue-router";

const route = useRoute();
const contacts = useContactsStore();
const activeContactId = () => route.params.contactId as string | undefined;

onMounted(() => {
  contacts.load();
});
</script>

<template>
  <div class="chat-view">
    <aside class="sidebar">
      <ConversationList />
    </aside>
    <main class="thread">
      <MessageThread v-if="activeContactId()" :contact-id="activeContactId()!" />
      <div v-else class="empty-state">
        <p>Select a conversation or start a new one</p>
      </div>
    </main>
  </div>
</template>

<style scoped>
.chat-view {
  display: flex;
  height: 100vh;
  background: var(--bg-primary);
}
.sidebar {
  width: 280px;
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.thread {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.empty-state {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
}
</style>
