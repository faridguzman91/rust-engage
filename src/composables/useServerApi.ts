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
  ephemeralKey?: string;
  otpkId?: number;
  ciphertext: string;
}

function getToken(): string | null {
  return localStorage.getItem("engage_jwt");
}

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
    // Token expired — clear it so the router redirects to login
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

export function useServerApi() {
  async function register(payload: RegisterPayload): Promise<void> {
    return request("POST", "/api/register", payload);
  }

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

  async function fetchOpkCount(userId: string): Promise<{ remaining: number }> {
    return request("GET", `/api/keys/${encodeURIComponent(userId)}/prekeys/count`);
  }

  return { register, fetchPreKeyBundle, uploadPreKeys, sendEnvelope, fetchPendingMessages, fetchOpkCount };
}
