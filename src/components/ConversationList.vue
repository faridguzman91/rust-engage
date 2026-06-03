<script setup lang="ts">
import { ref } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useContactsStore } from "../stores/contacts";
import { useIdentityStore } from "../stores/identity";
import Button from "primevue/button";
import InputText from "primevue/inputtext";
import Dialog from "primevue/dialog";
import Message from "primevue/message";
import Avatar from "primevue/avatar";
import Divider from "primevue/divider";
import FloatLabel from "primevue/floatlabel";

const router = useRouter();
const route = useRoute();
const contacts = useContactsStore();
const identity = useIdentityStore();

const showDialog = ref(false);
const newContactKey = ref("");
const newContactName = ref("");
const addError = ref("");
const adding = ref(false);

async function addContact() {
  if (!newContactName.value.trim() || !newContactKey.value.trim()) return;
  adding.value = true;
  addError.value = "";
  try {
    const contact = await contacts.addContact(newContactKey.value.trim(), newContactName.value.trim());
    showDialog.value = false;
    newContactKey.value = "";
    newContactName.value = "";
    router.push(`/chat/${contact.id}`);
  } catch (e) {
    addError.value = String(e);
  } finally {
    adding.value = false;
  }
}

function avatarLabel(name: string) {
  return name?.[0]?.toUpperCase() ?? "?";
}
</script>

<template>
  <div class="conv-list">
    <!-- Header -->
    <div class="conv-header">
      <span class="brand">engage</span>
      <div class="header-actions">
        <Button
          icon="pi pi-pencil"
          text rounded size="small"
          v-tooltip.bottom="'New conversation'"
          @click="showDialog = true"
        />
        <Button
          icon="pi pi-cog"
          text rounded size="small"
          v-tooltip.bottom="'Settings'"
          @click="router.push('/settings')"
        />
      </div>
    </div>

    <!-- Own identity chip -->
    <div class="self-row">
      <Avatar
        :label="avatarLabel(identity.displayName)"
        shape="circle"
        size="normal"
        style="background: var(--engage-accent); color: #fff; font-weight:700; font-size:0.85rem;"
      />
      <div class="self-info">
        <span class="self-name">{{ identity.displayName }}</span>
        <span class="self-tag">You</span>
      </div>
    </div>

    <Divider style="margin: 0;" />

    <!-- Contact list -->
    <div class="contacts-scroll">
      <div
        v-for="contact in contacts.contacts"
        :key="contact.id"
        class="contact-row"
        :class="{ active: route.params.contactId === contact.id }"
        @click="router.push(`/chat/${contact.id}`)"
      >
        <Avatar
          :label="avatarLabel(contact.displayName)"
          shape="circle"
          size="normal"
          style="background: #4a4a78; color: #e8eaf6; font-weight:600; flex-shrink:0;"
        />
        <div class="contact-info">
          <span class="contact-name truncate">{{ contact.displayName }}</span>
        </div>
      </div>

      <div v-if="contacts.contacts.length === 0" class="no-contacts">
        <i class="pi pi-user-plus" style="font-size:1.5rem; opacity:0.3;" />
        <p>No contacts yet.<br />Add one to start chatting.</p>
      </div>
    </div>
  </div>

  <!-- Add contact dialog -->
  <Dialog
    v-model:visible="showDialog"
    header="New conversation"
    :style="{ width: '380px' }"
    :modal="true"
    :draggable="false"
  >
    <div class="dialog-body">
      <FloatLabel variant="on">
        <InputText id="c-name" v-model="newContactName" class="w-full" />
        <label for="c-name">Display name</label>
      </FloatLabel>
      <FloatLabel variant="on">
        <InputText id="c-key" v-model="newContactKey" class="w-full" />
        <label for="c-key">Identity public key</label>
      </FloatLabel>
      <Message v-if="addError" severity="error" :closable="false">{{ addError }}</Message>
    </div>
    <template #footer>
      <Button label="Cancel" text @click="showDialog = false" />
      <Button
        label="Add contact"
        icon="pi pi-user-plus"
        :loading="adding"
        :disabled="!newContactName.trim() || !newContactKey.trim()"
        @click="addContact"
      />
    </template>
  </Dialog>
</template>

<style scoped>
.conv-list {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--engage-sidebar-bg);
  overflow: hidden;
}
.conv-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.85rem 1rem;
  flex-shrink: 0;
}
.brand {
  font-weight: 800;
  font-size: 1.15rem;
  color: var(--engage-accent);
  letter-spacing: -0.02em;
}
.header-actions { display: flex; gap: 0.25rem; }
.self-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.6rem 1rem;
  flex-shrink: 0;
}
.self-info { display: flex; flex-direction: column; min-width: 0; }
.self-name { font-size: 0.9rem; font-weight: 600; color: var(--engage-text); }
.self-tag { font-size: 0.72rem; color: var(--engage-accent); font-weight: 500; }
.contacts-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 0.25rem 0;
}
.contact-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.7rem 1rem;
  cursor: pointer;
  transition: background 0.1s;
  border-radius: 0;
}
.contact-row:hover { background: var(--engage-sidebar-hover); }
.contact-row.active { background: var(--engage-sidebar-active); }
.contact-info { display: flex; flex-direction: column; min-width: 0; flex: 1; }
.contact-name { font-size: 0.92rem; font-weight: 500; color: var(--engage-text); }
.no-contacts {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  padding: 3rem 1rem;
  color: var(--engage-muted);
  text-align: center;
  font-size: 0.85rem;
  line-height: 1.6;
}
.dialog-body { display: flex; flex-direction: column; gap: 1.25rem; padding: 0.25rem 0 0.5rem; }
.w-full { width: 100%; }
</style>
