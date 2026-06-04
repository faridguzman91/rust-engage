<script setup lang="ts">
import { onMounted } from "vue";
import { useRoute } from "vue-router";
import { useContactsStore } from "../stores/contacts";
import ConversationList from "../components/ConversationList.vue";
import MessageThread from "../components/MessageThread.vue";

const route = useRoute();
const contacts = useContactsStore();
const activeContactId = () => route.params.contactId as string | undefined;

onMounted(() => contacts.load());
</script>

<template>
  <div class="chat-layout">
    <aside class="sidebar">
      <ConversationList />
    </aside>
    <main class="thread-pane">
      <MessageThread v-if="activeContactId()" :contact-id="activeContactId()!" />
      <div v-else class="empty-state">
        <i class="pi pi-comments empty-icon" />
        <p>Select a conversation or<br />add a new contact</p>
      </div>
    </main>
  </div>
</template>

<style scoped>
.chat-layout {
  display: flex;
  height: 100vh;
  background: var(--engage-main-bg);
  overflow: hidden;
}
.sidebar {
  width: 300px;
  flex-shrink: 0;
  border-right: 1px solid var(--engage-border);
  display: flex;
  flex-direction: column;
  background: var(--engage-sidebar-bg);
  overflow: hidden;
}
.thread-pane {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1rem;
  color: var(--engage-muted);
}
.empty-icon {
  font-size: 3rem;
  opacity: 0.25;
}
</style>
