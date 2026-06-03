<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useContactsStore } from "../stores/contacts";
import { useIdentityStore } from "../stores/identity";
import { useGroupsStore } from "../stores/groups";
import Button from "primevue/button";
import InputText from "primevue/inputtext";
import Dialog from "primevue/dialog";
import Message from "primevue/message";
import Avatar from "primevue/avatar";
import Divider from "primevue/divider";
import FloatLabel from "primevue/floatlabel";
import Tabs from "primevue/tabs";
import Tab from "primevue/tab";
import TabList from "primevue/tablist";
import TabPanels from "primevue/tabpanels";
import TabPanel from "primevue/tabpanel";

const router = useRouter();
const route = useRoute();
const contacts = useContactsStore();
const identity = useIdentityStore();
const groups = useGroupsStore();

// @faridguzman91: Contact dialog
const showContactDialog = ref(false);
const newContactKey = ref("");
const newContactName = ref("");
const addError = ref("");
const adding = ref(false);

// @faridguzman91: Group dialog
const showGroupDialog = ref(false);
const newGroupName = ref("");
const selectedMemberIds = ref<string[]>([]);
const groupError = ref("");
const creatingGroup = ref(false);

onMounted(async () => {
  await groups.load();
});

async function addContact() {
  if (!newContactName.value.trim() || !newContactKey.value.trim()) return;
  adding.value = true;
  addError.value = "";
  try {
    const contact = await contacts.addContact(newContactKey.value.trim(), newContactName.value.trim());
    showContactDialog.value = false;
    newContactKey.value = "";
    newContactName.value = "";
    router.push(`/chat/${contact.id}`);
  } catch (e) {
    addError.value = String(e);
  } finally {
    adding.value = false;
  }
}

async function createGroup() {
  if (!newGroupName.value.trim()) return;
  creatingGroup.value = true;
  groupError.value = "";
  try {
    const group = await groups.create(newGroupName.value.trim(), selectedMemberIds.value);
    showGroupDialog.value = false;
    newGroupName.value = "";
    selectedMemberIds.value = [];
    router.push(`/group/${group.id}`);
  } catch (e) {
    groupError.value = String(e);
  } finally {
    creatingGroup.value = false;
  }
}

function toggleMember(id: string) {
  const idx = selectedMemberIds.value.indexOf(id);
  if (idx === -1) selectedMemberIds.value.push(id);
  else selectedMemberIds.value.splice(idx, 1);
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
          @click="showContactDialog = true"
        />
        <Button
          icon="pi pi-users"
          text rounded size="small"
          v-tooltip.bottom="'New group'"
          @click="showGroupDialog = true"
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

    <!-- @faridguzman91: Tabbed view — Direct / Groups -->
    <Tabs value="direct" class="sidebar-tabs">
      <TabList>
        <Tab value="direct">Direct</Tab>
        <Tab value="groups">Groups</Tab>
      </TabList>

      <TabPanels>
        <!-- Direct messages -->
        <TabPanel value="direct">
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
            <div v-if="contacts.contacts.length === 0" class="no-items">
              <i class="pi pi-user-plus" style="font-size:1.5rem; opacity:0.3;" />
              <p>No contacts yet.<br />Add one to start chatting.</p>
            </div>
          </div>
        </TabPanel>

        <!-- Groups -->
        <TabPanel value="groups">
          <div class="contacts-scroll">
            <div
              v-for="group in groups.groups"
              :key="group.id"
              class="contact-row"
              :class="{ active: route.params.groupId === group.id }"
              @click="router.push(`/group/${group.id}`)"
            >
              <div class="group-avatar">
                <i class="pi pi-users" style="font-size:0.9rem;" />
              </div>
              <div class="contact-info">
                <span class="contact-name truncate">{{ group.name }}</span>
                <span class="contact-sub">{{ group.members.length }} members</span>
              </div>
            </div>
            <div v-if="groups.groups.length === 0" class="no-items">
              <i class="pi pi-users" style="font-size:1.5rem; opacity:0.3;" />
              <p>No groups yet.<br />Create one to get started.</p>
            </div>
          </div>
        </TabPanel>
      </TabPanels>
    </Tabs>
  </div>

  <!-- Add contact dialog -->
  <Dialog
    v-model:visible="showContactDialog"
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
      <Button label="Cancel" text @click="showContactDialog = false" />
      <Button
        label="Add contact"
        icon="pi pi-user-plus"
        :loading="adding"
        :disabled="!newContactName.trim() || !newContactKey.trim()"
        @click="addContact"
      />
    </template>
  </Dialog>

  <!-- New group dialog -->
  <Dialog
    v-model:visible="showGroupDialog"
    header="New group"
    :style="{ width: '400px' }"
    :modal="true"
    :draggable="false"
  >
    <div class="dialog-body">
      <FloatLabel variant="on">
        <InputText id="g-name" v-model="newGroupName" class="w-full" />
        <label for="g-name">Group name</label>
      </FloatLabel>

      <div v-if="contacts.contacts.length > 0">
        <p class="member-pick-label">Add members</p>
        <div class="member-pick-list">
          <div
            v-for="c in contacts.contacts"
            :key="c.id"
            class="member-pick-row"
            :class="{ selected: selectedMemberIds.includes(c.id) }"
            @click="toggleMember(c.id)"
          >
            <Avatar :label="avatarLabel(c.displayName)" shape="circle" size="small"
              style="background:#4a4a78;color:#e8eaf6;font-weight:600;flex-shrink:0;" />
            <span class="member-pick-name">{{ c.displayName }}</span>
            <i v-if="selectedMemberIds.includes(c.id)" class="pi pi-check" style="color:var(--engage-accent);margin-left:auto;" />
          </div>
        </div>
      </div>

      <Message v-if="groupError" severity="error" :closable="false">{{ groupError }}</Message>
    </div>
    <template #footer>
      <Button label="Cancel" text @click="showGroupDialog = false" />
      <Button
        label="Create group"
        icon="pi pi-users"
        :loading="creatingGroup"
        :disabled="!newGroupName.trim()"
        @click="createGroup"
      />
    </template>
  </Dialog>
</template>

<style scoped>
.conv-list { display:flex; flex-direction:column; height:100%; background:var(--engage-sidebar-bg); overflow:hidden; }
.conv-header { display:flex; align-items:center; justify-content:space-between; padding:0.85rem 1rem; flex-shrink:0; }
.brand { font-weight:800; font-size:1.15rem; color:var(--engage-accent); letter-spacing:-0.02em; }
.header-actions { display:flex; gap:0.25rem; }
.self-row { display:flex; align-items:center; gap:0.75rem; padding:0.6rem 1rem; flex-shrink:0; }
.self-info { display:flex; flex-direction:column; min-width:0; }
.self-name { font-size:0.9rem; font-weight:600; color:var(--engage-text); }
.self-tag { font-size:0.72rem; color:var(--engage-accent); font-weight:500; }

.sidebar-tabs { flex:1; display:flex; flex-direction:column; overflow:hidden; }
:deep(.p-tabs) { flex:1; display:flex; flex-direction:column; }
:deep(.p-tabpanels) { flex:1; overflow:hidden; padding:0; }
:deep(.p-tabpanel) { height:100%; padding:0; }

.contacts-scroll { height:100%; overflow-y:auto; padding:0.25rem 0; }
.contact-row { display:flex; align-items:center; gap:0.75rem; padding:0.7rem 1rem; cursor:pointer; transition:background 0.1s; }
.contact-row:hover { background:var(--engage-sidebar-hover); }
.contact-row.active { background:var(--engage-sidebar-active); }
.contact-info { display:flex; flex-direction:column; min-width:0; flex:1; }
.contact-name { font-size:0.92rem; font-weight:500; color:var(--engage-text); }
.contact-sub { font-size:0.72rem; color:var(--engage-muted); }
.group-avatar { width:36px; height:36px; border-radius:50%; background:var(--engage-accent); color:#fff; display:flex; align-items:center; justify-content:center; flex-shrink:0; }
.no-items { display:flex; flex-direction:column; align-items:center; justify-content:center; gap:0.75rem; padding:3rem 1rem; color:var(--engage-muted); text-align:center; font-size:0.85rem; line-height:1.6; }

.dialog-body { display:flex; flex-direction:column; gap:1.25rem; padding:0.25rem 0 0.5rem; }
.w-full { width:100%; }
.member-pick-label { font-size:0.8rem; color:var(--engage-muted); margin:0; }
.member-pick-list { display:flex; flex-direction:column; gap:0.25rem; max-height:200px; overflow-y:auto; }
.member-pick-row { display:flex; align-items:center; gap:0.6rem; padding:0.4rem 0.5rem; border-radius:6px; cursor:pointer; transition:background 0.1s; }
.member-pick-row:hover { background:var(--engage-sidebar-hover); }
.member-pick-row.selected { background:var(--engage-sidebar-active); }
.member-pick-name { font-size:0.88rem; color:var(--engage-text); }
</style>
