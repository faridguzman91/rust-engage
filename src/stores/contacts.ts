import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useServerApi } from "../composables/useServerApi";
import { useOpkReplenishment } from "../composables/useOpkReplenishment";

export interface Contact {
  id: string;
  displayName: string;
  identityPublicKey: string;
  lastSeen?: number;
}

export const useContactsStore = defineStore("contacts", () => {
  const contacts = ref<Contact[]>([]);
  // Contacts with an established ratchet session → their ephemeral key (set after X3DH)
  const ephemeralKeys = ref<Record<string, string>>({});

  async function load() {
    contacts.value = await invoke<Contact[]>("list_contacts");
  }

  async function addContact(identityKey: string, displayName: string): Promise<Contact> {
    const contact = await invoke<Contact>("add_contact", { identityKey, displayName });
    contacts.value.push(contact);
    return contact;
  }

  async function removeContact(id: string) {
    await invoke("remove_contact", { id });
    contacts.value = contacts.value.filter((c) => c.id !== id);
    delete ephemeralKeys.value[id];
  }

  /**
   * Ensure a ratchet session exists for this contact.
   * Returns the ephemeral key string on first call (to be sent with the first message)
   * so the recipient can complete X3DH and derive the same shared secret.
   * Returns null on subsequent calls.
   */
  async function ensureSession(contactId: string): Promise<string | null> {
    if (ephemeralKeys.value[contactId] !== undefined) {
      // Already established — consume and clear the pending ephemeral key (only sent once)
      const ek = ephemeralKeys.value[contactId] || null;
      if (ek) {
        ephemeralKeys.value[contactId] = ""; // mark as consumed
        return ek;
      }
      return null;
    }

    // First time: fetch remote bundle and do X3DH
    const api = useServerApi();
    const bundle = await api.fetchPreKeyBundle(contactId);

    // Rust performs X3DH → Double Ratchet init, returns the ephemeral key
    const ek = await invoke<string>("init_session", { contactId, bundle });

    // Store the ephemeral key so the first send can include it
    ephemeralKeys.value[contactId] = ek;

    // One OPK was just consumed from the server pool — check if we need to replenish
    useOpkReplenishment().checkAndReplenish();

    return ek;
  }

  function hasSession(contactId: string): boolean {
    return contactId in ephemeralKeys.value;
  }

  function getById(id: string): Contact | undefined {
    return contacts.value.find((c) => c.id === id);
  }

  return { contacts, load, addContact, removeContact, ensureSession, hasSession, getById };
});
