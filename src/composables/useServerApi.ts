import { SERVER_BASE } from "../config";
import type { PreKeyBundle } from "./useCrypto";

export interface RegisterPayload {
  userId: string;
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

async function request<T>(
  method: string,
  path: string,
  body?: unknown
): Promise<T> {
  const res = await fetch(`${SERVER_BASE}${path}`, {
    method,
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
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

  async function uploadPreKeys(
    userId: string,
    keys: { keyId: number; publicKey: string }[]
  ): Promise<void> {
    return request("POST", `/api/keys/${encodeURIComponent(userId)}/prekeys`, keys);
  }

  async function sendEnvelope(payload: SendEnvelopePayload): Promise<void> {
    return request("POST", "/api/messages", payload);
  }

  async function fetchPendingMessages(userId: string): Promise<unknown[]> {
    return request("GET", `/api/messages/${encodeURIComponent(userId)}`);
  }

  return { register, fetchPreKeyBundle, uploadPreKeys, sendEnvelope, fetchPendingMessages };
}
