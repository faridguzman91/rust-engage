// @faridguzman91: Contacts store — CRUD for the contact list plus X3DH session management.
// The first time a message is sent to a contact, ensureSession() fetches their prekey bundle
// from the server, runs X3DH locally, and seeds the Double Ratchet. The ephemeral key
// produced during X3DH is sent with the first message so the recipient can mirror the exchange.
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

  // @faridguzman91: ephemeralKeys maps contactId → base64 EK_A produced during X3DH init.
  // The key is included in the first message envelope so the recipient can complete X3DH.
  // After it's consumed (sent once), the entry is set to "" to indicate "session exists but EK sent".
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
   * @faridguzman91: Ensure a Double Ratchet session exists for this contact.
   *
   * First call  → fetches remote prekey bundle, runs X3DH, seeds the ratchet.
   *               Returns the EK_A (ephemeral key) to include in the first message.
   * Second call → session already exists, EK_A already sent. Returns null.
   *
   * Also triggers OPK replenishment after a session is initiated since one OPK
   * was just consumed from the server pool.
   */
  async function ensureSession(contactId: string): Promise<string | null> {
    if (ephemeralKeys.value[contactId] !== undefined) {
      const ek = ephemeralKeys.value[contactId] || null;
      if (ek) {
        ephemeralKeys.value[contactId] = ""; // mark EK as consumed
        return ek;
      }
      return null;
    }

    // First time — fetch remote bundle and run X3DH on the Rust side
    const api = useServerApi();
    const bundle = await api.fetchPreKeyBundle(contactId);
    const ek = await invoke<string>("init_session", { contactId, bundle });
    ephemeralKeys.value[contactId] = ek;

    // @faridguzman91: One OPK was consumed — check if replenishment is needed
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
