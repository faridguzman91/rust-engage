// @faridguzman91: Typed fetch wrapper for the relay server.
// Automatically attaches the Bearer JWT to every request.
// On 401 (token expired/revoked), clears localStorage and redirects to /login.
import { SERVER_BASE } from "../config";
import type { PreKeyBundle } from "./useCrypto";

export interface RegisterPayload {
  displayName: string;
  identityKey: string;
  signedPreKey: { keyId: number; publicKey: string; signature: string };
  oneTimePreKeys: { keyId: number; publicKey: string }[];
  registrationId: number;
}

export interface SendEnvelopePayload {
  recipientId: string;
  senderIk: string;
  ephemeralKey?: string;   // only present on the first message (X3DH initiator envelope)
  otpkId?: number;
  ciphertext: string;
}

function getToken(): string | null {
  return localStorage.getItem("engage_jwt");
}

// @faridguzman91: Generic request helper — handles JSON Content-Type, auth header,
// 401 redirect, and empty-body (204) responses.
async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = {};
  if (body) headers["Content-Type"] = "application/json";
  if (token) headers["Authorization"] = `Bearer ${token}`;

  const res = await fetch(`${SERVER_BASE}${path}`, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });

  if (res.status === 401) {
    localStorage.removeItem("engage_jwt");
    window.location.hash = "#/login";
    throw new Error("session expired");
  }
  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(`${method} ${path} → ${res.status}: ${text}`);
  }
  if (res.status === 204 || res.headers.get("content-length") === "0") {
    return undefined as T;
  }
  return res.json();
}

export interface ContactSuggestion {
  userId: string;
  displayName: string;
  identityKey: string;
  email: string;
}

export function useServerApi() {
  // @faridguzman91: Register uploads public crypto keys after first identity creation.
  // The server derives user_id from the JWT — the body cannot override it.
  async function register(payload: RegisterPayload): Promise<void> {
    return request("POST", "/api/register", payload);
  }

  // @faridguzman91: Fetches a remote contact's prekey bundle for X3DH session init.
  // The server atomically marks one OPK as used so it is never served twice.
  async function fetchPreKeyBundle(userId: string): Promise<PreKeyBundle> {
    return request("GET", `/api/keys/${encodeURIComponent(userId)}`);
  }

  async function uploadPreKeys(userId: string, keys: { keyId: number; publicKey: string }[]): Promise<void> {
    return request("POST", `/api/keys/${encodeURIComponent(userId)}/prekeys`, keys);
  }

  async function sendEnvelope(payload: SendEnvelopePayload): Promise<void> {
    return request("POST", "/api/messages", payload);
  }

  async function fetchPendingMessages(userId: string): Promise<unknown[]> {
    return request("GET", `/api/messages/${encodeURIComponent(userId)}`);
  }

  // @faridguzman91: Returns how many unused OPKs the server holds for us.
  async function fetchOpkCount(userId: string): Promise<{ remaining: number }> {
    return request("GET", `/api/keys/${encodeURIComponent(userId)}/prekeys/count`);
  }

  // ── Group API ───────────────────────────────────────────────────────────────

  async function createGroup(payload: { name: string; members: string[] }) {
    return request<import("../stores/groups").Group>("POST", "/api/groups", payload);
  }

  async function listGroups() {
    return request<import("../stores/groups").Group[]>("GET", "/api/groups");
  }

  async function getGroup(groupId: string) {
    return request<import("../stores/groups").Group>("GET", `/api/groups/${encodeURIComponent(groupId)}`);
  }

  async function addGroupMember(groupId: string, userId: string) {
    return request<import("../stores/groups").GroupMember>(
      "POST",
      `/api/groups/${encodeURIComponent(groupId)}/members`,
      { userId }
    );
  }

  async function removeGroupMember(groupId: string, userId: string) {
    return request<void>(
      "DELETE",
      `/api/groups/${encodeURIComponent(groupId)}/members/${encodeURIComponent(userId)}`
    );
  }

  async function sendGroupEnvelope(groupId: string, payload: { senderIk: string; ciphertext: string }) {
    return request<void>("POST", `/api/groups/${encodeURIComponent(groupId)}/messages`, {
      groupId,
      ...payload,
    });
  }

  async function suggestContacts(): Promise<ContactSuggestion[]> {
    return request("GET", "/api/contacts/suggest");
  }

  return {
    register, fetchPreKeyBundle, uploadPreKeys, sendEnvelope, fetchPendingMessages, fetchOpkCount,
    createGroup, listGroups, getGroup, addGroupMember, removeGroupMember, sendGroupEnvelope,
    suggestContacts,
  };
}
