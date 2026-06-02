<script setup lang="ts">
import { ref } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useContactsStore } from "../stores/contacts";
import { useIdentityStore } from "../stores/identity";

const router = useRouter();
const route = useRoute();
const contacts = useContactsStore();
const identity = useIdentityStore();

const showAddContact = ref(false);
const newContactKey = ref("");
const newContactName = ref("");
const addError = ref("");

async function addContact() {
  addError.value = "";
  try {
    const contact = await contacts.addContact(newContactKey.value.trim(), newContactName.value.trim());
    showAddContact.value = false;
    newContactKey.value = "";
    newContactName.value = "";
    router.push(`/chat/${contact.id}`);
  } catch (e) {
    addError.value = String(e);
  }
}
</script>

<template>
  <div class="conversation-list">
    <header class="list-header">
      <span class="app-name">engage</span>
      <div class="header-actions">
        <button class="icon-btn" title="New conversation" @click="showAddContact = true">+</button>
        <router-link to="/settings" class="icon-btn" title="Settings">⚙</router-link>
      </div>
    </header>

    <div v-if="showAddContact" class="add-contact-form">
      <input v-model="newContactName" placeholder="Display name" />
      <input v-model="newContactKey" placeholder="Identity public key" />
      <p v-if="addError" class="error">{{ addError }}</p>
      <div class="form-actions">
        <button @click="addContact">Add</button>
        <button class="cancel" @click="showAddContact = false">Cancel</button>
      </div>
    </div>

    <div class="identity-bar">
      <span class="identity-name">{{ identity.displayName }}</span>
    </div>

    <ul class="contacts">
      <li
        v-for="contact in contacts.contacts"
        :key="contact.id"
        :class="{ active: route.params.contactId === contact.id }"
        @click="router.push(`/chat/${contact.id}`)"
      >
        <div class="avatar">{{ contact.displayName[0]?.toUpperCase() }}</div>
        <div class="contact-info">
          <span class="contact-name">{{ contact.displayName }}</span>
        </div>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.conversation-list {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-secondary);
}
.list-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1rem;
  border-bottom: 1px solid var(--border);
}
.app-name {
  font-weight: 700;
  font-size: 1.1rem;
  color: var(--accent);
}
.header-actions {
  display: flex;
  gap: 0.5rem;
}
.icon-btn {
  background: none;
  border: none;
  cursor: pointer;
  font-size: 1.1rem;
  color: var(--text-muted);
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
  text-decoration: none;
}
.icon-btn:hover { background: var(--bg-hover); }
.identity-bar {
  padding: 0.5rem 1rem;
  font-size: 0.8rem;
  color: var(--text-muted);
  border-bottom: 1px solid var(--border);
}
.contacts {
  list-style: none;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  flex: 1;
}
.contacts li {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.75rem 1rem;
  cursor: pointer;
  transition: background 0.1s;
}
.contacts li:hover { background: var(--bg-hover); }
.contacts li.active { background: var(--bg-active); }
.avatar {
  width: 38px;
  height: 38px;
  border-radius: 50%;
  background: var(--accent);
  color: #fff;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 600;
  font-size: 1rem;
  flex-shrink: 0;
}
.contact-name { font-size: 0.95rem; }
.add-contact-form {
  padding: 0.75rem 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  border-bottom: 1px solid var(--border);
}
.add-contact-form input {
  padding: 0.4rem 0.6rem;
  border-radius: 6px;
  border: 1px solid var(--border);
  background: var(--bg-input);
  color: var(--text-primary);
  font-size: 0.9rem;
}
.form-actions { display: flex; gap: 0.5rem; }
.form-actions button {
  flex: 1;
  padding: 0.4rem;
  border-radius: 6px;
  border: none;
  cursor: pointer;
  background: var(--accent);
  color: #fff;
  font-size: 0.85rem;
}
.form-actions button.cancel {
  background: var(--bg-hover);
  color: var(--text-primary);
}
.error { font-size: 0.8rem; color: var(--danger); }
</style>
