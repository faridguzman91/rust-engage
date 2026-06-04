// @faridguzman91: Groups store — manages group chat state.
//
// Groups use Sender Keys (see crypto/sender_key.rs):
//   - Each member generates their own Sender Key and distributes it to all others
//   - Group messages are encrypted once with the Sender Key (not N pairwise sessions)
//   - After distributing our key, we can send; after receiving a key, we can decrypt
//
// The server is the source of truth for group membership.
// We cache group metadata locally for display (list_cached_groups / cache_group).
import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useServerApi } from "../composables/useServerApi";
import { useIdentityStore } from "./identity";
import { useAuthStore } from "./auth";
import { useContactsStore } from "./contacts";

export interface GroupMember {
  userId: string;
  displayName: string;
  identityKey: string;
}

export interface Group {
  id: string;
  name: string;
  createdBy: string;
  createdAt: number;
  members: GroupMember[];
}

// @faridguzman91: Track which groups we have already distributed our Sender Key to.
// Persisted in-memory only — on restart we re-distribute if needed (server tracks
// whether a member has received our key via message history).
const distributedTo = new Set<string>(); // groupId

export const useGroupsStore = defineStore("groups", () => {
  const groups = ref<Group[]>([]);

  const api = useServerApi();

  async function load() {
    // Fetch from server and cache locally
    const serverGroups = await api.listGroups();
    groups.value = serverGroups;
    for (const g of serverGroups) {
      await invoke("cache_group", {
        id: g.id,
        name: g.name,
        createdBy: g.createdBy,
      });
    }
  }

  async function create(name: string, memberIds: string[]): Promise<Group> {
    const group = await api.createGroup({ name, members: memberIds });
    groups.value.push(group);
    await invoke("cache_group", { id: group.id, name: group.name, createdBy: group.createdBy });

    // Distribute our Sender Key to all initial members
    await distributeSenderKey(group);
    return group;
  }

  async function addMember(groupId: string, userId: string): Promise<GroupMember> {
    const member = await api.addGroupMember(groupId, userId);
    const group = groups.value.find((g) => g.id === groupId);
    if (group) group.members.push(member);

    // Distribute our Sender Key to the new member via their pairwise session
    await distributeSenderKeyToMember(groupId, userId);
    return member;
  }

  async function leave(groupId: string) {
    const auth = useAuthStore();
    const userId = auth.profile?.userId ?? "";
    if (!userId) return;
    await api.removeGroupMember(groupId, userId);
    groups.value = groups.value.filter((g) => g.id !== groupId);
  }

  // @faridguzman91: Encrypt a message with our Sender Key.
  // Before the first message, ensure our Sender Key has been distributed.
  async function encryptMessage(groupId: string, plaintext: string): Promise<string> {
    const auth = useAuthStore();
    const userId = auth.profile?.userId ?? "";

    // Distribute our Sender Key to any members who don't have it yet
    const group = groups.value.find((g) => g.id === groupId);
    if (group && !distributedTo.has(groupId)) {
      await distributeSenderKey(group);
    }

    return invoke<string>("encrypt_group_message", { groupId, ourUserId: userId, plaintext });
  }

  async function decryptMessage(
    groupId: string,
    senderId: string,
    ciphertext: string
  ): Promise<string> {
    return invoke<string>("decrypt_group_message", { groupId, senderId, ciphertext });
  }

  // @faridguzman91: Get our Sender Key distribution JSON and send it to each member
  // via their pairwise ratchet session (encrypted, so the server can't read it).
  async function distributeSenderKey(group: Group) {
    const auth = useAuthStore();
    const userId = auth.profile?.userId ?? "";
    const contacts = useContactsStore();

    const distJson = await invoke<string>("get_sender_key_distribution", {
      groupId: group.id,
      ourUserId: userId,
    });

    for (const member of group.members) {
      if (member.userId === userId) continue;
      await distributeSenderKeyToMemberWithDist(group.id, member.userId, distJson, contacts);
    }
    distributedTo.add(group.id);
  }

  async function distributeSenderKeyToMember(groupId: string, memberId: string) {
    const auth = useAuthStore();
    const userId = auth.profile?.userId ?? "";
    const contacts = useContactsStore();

    const distJson = await invoke<string>("get_sender_key_distribution", {
      groupId,
      ourUserId: userId,
    });
    await distributeSenderKeyToMemberWithDist(groupId, memberId, distJson, contacts);
  }

  async function distributeSenderKeyToMemberWithDist(
    groupId: string,
    memberId: string,
    distJson: string,
    contacts: ReturnType<typeof useContactsStore>
  ) {
    // @faridguzman91: Wrap the distribution JSON in a control envelope, encrypt it
    // via the pairwise ratchet, and send it as a special message type.
    try {
      await contacts.ensureSession(memberId);
      const { useMessagesStore } = await import("./messages");
      const messages = useMessagesStore();
      // Send as a "sender_key_distribution" control message — the recipient
      // will decode and store it without displaying it in the chat thread.
      await messages.sendControl(memberId, {
        type: "sender_key_distribution",
        groupId,
        payload: distJson,
      });
    } catch (e) {
      console.warn(`[groups] Failed to distribute sender key to ${memberId}:`, e);
    }
  }

  // @faridguzman91: Handle an incoming Sender Key distribution from another member.
  async function receiveDistribution(distJson: string) {
    await invoke("store_received_sender_key", { distributionJson: distJson });
  }

  function getById(id: string): Group | undefined {
    return groups.value.find((g) => g.id === id);
  }

  return {
    groups,
    load,
    create,
    addMember,
    leave,
    encryptMessage,
    decryptMessage,
    receiveDistribution,
    getById,
  };
});
