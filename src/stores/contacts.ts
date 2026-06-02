import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface Contact {
  id: string;
  displayName: string;
  identityPublicKey: string;
  lastSeen?: number;
}

export const useContactsStore = defineStore("contacts", () => {
  const contacts = ref<Contact[]>([]);

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
  }

  function getById(id: string): Contact | undefined {
    return contacts.value.find((c) => c.id === id);
  }

  return { contacts, load, addContact, removeContact, getById };
});
