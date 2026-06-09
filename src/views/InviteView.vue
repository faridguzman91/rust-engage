<script setup lang="ts">
// @faridguzman: Invite acceptance screen.
// Opened when the user taps an engage://invite?token=TOKEN deep link or
// navigates directly to /#/invite?token=TOKEN.
// Fetches the inviter's identity bundle, shows a confirmation card, and on
// accept calls addContact locally then navigates to the new conversation.
import { ref, onMounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import { useContactsStore } from "../stores/contacts";
import { useServerApi } from "../composables/useServerApi";
import Avatar from "primevue/avatar";
import Button from "primevue/button";
import Card from "primevue/card";
import ProgressSpinner from "primevue/progressspinner";
import Tag from "primevue/tag";
import Message from "primevue/message";

const router = useRouter();
const route  = useRoute();
const contacts = useContactsStore();
const api = useServerApi();

type InviterBundle = { userId: string; displayName: string; identityKey: string };

const state = ref<"loading" | "ready" | "adding" | "error">("loading");
const inviter = ref<InviterBundle | null>(null);
const errorMsg = ref("");

onMounted(async () => {
  const token = route.query.token as string | undefined;
  if (!token) {
    errorMsg.value = "Invalid invite link — no token found.";
    state.value = "error";
    return;
  }

  try {
    inviter.value = await api.redeemInvite(token);
    state.value = "ready";
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    if (msg.includes("410") || msg.includes("Gone")) {
      errorMsg.value = "This invite has already been used or has expired.";
    } else if (msg.includes("404") || msg.includes("Not Found")) {
      errorMsg.value = "Invite not found. The link may be invalid.";
    } else {
      errorMsg.value = "Could not load invite. Please try again.";
    }
    state.value = "error";
  }
});

async function accept() {
  if (!inviter.value) return;
  state.value = "adding";
  try {
    const contact = await contacts.addContact(inviter.value.identityKey, inviter.value.displayName);
    // @faridguzman: Navigate directly into the new conversation
    router.replace(`/chat/${contact.id}`);
  } catch {
    errorMsg.value = "Failed to add contact. They may already be in your list.";
    state.value = "error";
  }
}

function avatarLabel(name: string) {
  return name[0]?.toUpperCase() ?? "?";
}
</script>

<template>
  <div class="invite-wrap">
    <!-- Loading -->
    <div v-if="state === 'loading'" class="center-state">
      <ProgressSpinner style="width:48px;height:48px;" strokeWidth="4" />
      <p>Loading invite…</p>
    </div>

    <!-- Error -->
    <div v-else-if="state === 'error'" class="center-state">
      <i class="pi pi-exclamation-circle" style="font-size:2.5rem; color: var(--p-red-400);" />
      <Message severity="error" :closable="false" style="max-width:360px; text-align:center;">
        {{ errorMsg }}
      </Message>
      <Button label="Go to chat" icon="pi pi-arrow-left" text @click="router.replace('/chat')" />
    </div>

    <!-- Ready to accept -->
    <div v-else class="center-state">
      <Card class="invite-card">
        <template #header>
          <div class="invite-header">
            <i class="pi pi-user-plus invite-icon" />
            <p class="invite-heading">You've been invited to connect</p>
          </div>
        </template>

        <template #content>
          <div class="inviter-row">
            <Avatar
              :label="avatarLabel(inviter!.displayName)"
              shape="circle"
              size="large"
              style="background: var(--engage-accent); color: #fff; font-weight: 700; flex-shrink: 0;"
            />
            <div>
              <p class="inviter-name">{{ inviter!.displayName }}</p>
              <Tag value="End-to-end encrypted" icon="pi pi-lock" severity="success" style="font-size:0.68rem;" />
            </div>
          </div>

          <p class="invite-note">
            Adding this contact establishes an encrypted session using their
            identity key. Messages can only be read by the two of you.
          </p>

          <code class="ik-preview">{{ inviter!.identityKey.slice(0, 24) }}…</code>
        </template>

        <template #footer>
          <div class="invite-actions">
            <Button
              label="Decline"
              icon="pi pi-times"
              severity="secondary"
              outlined
              @click="router.replace('/chat')"
            />
            <Button
              label="Add contact"
              icon="pi pi-user-plus"
              :loading="state === 'adding'"
              style="background: var(--engage-accent); border-color: var(--engage-accent);"
              @click="accept"
            />
          </div>
        </template>
      </Card>
    </div>
  </div>
</template>

<style scoped>
.invite-wrap {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--engage-main-bg);
  padding: 1.5rem;
}
.center-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.25rem;
  color: var(--engage-muted);
  font-size: 0.9rem;
}
.invite-card {
  width: 100%;
  max-width: 400px;
  background: var(--engage-sidebar-bg);
  border: 1px solid var(--engage-border);
}
.invite-header {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.5rem;
  padding: 1.5rem 1.5rem 0;
}
.invite-icon {
  font-size: 2rem;
  color: var(--engage-accent);
}
.invite-heading {
  font-size: 1rem;
  font-weight: 600;
  margin: 0;
  text-align: center;
}
.inviter-row {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 1rem;
}
.inviter-name {
  font-weight: 600;
  font-size: 1rem;
  margin: 0 0 0.25rem;
}
.invite-note {
  font-size: 0.8rem;
  color: var(--engage-muted);
  line-height: 1.55;
  margin: 0 0 0.75rem;
}
.ik-preview {
  font-size: 0.7rem;
  color: var(--engage-muted);
  background: var(--engage-input-bg);
  padding: 0.3rem 0.6rem;
  border-radius: 4px;
  display: block;
  word-break: break-all;
}
.invite-actions {
  display: flex;
  gap: 0.75rem;
  justify-content: flex-end;
}
</style>
