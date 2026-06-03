// @faridguzman91: Thin TypeScript wrappers over the Tauri crypto commands.
// The actual cryptography runs in Rust (src-tauri/src/crypto/) where it has access
// to native OS entropy, persistent key storage, and no JavaScript sandbox limits.
import { invoke } from "@tauri-apps/api/core";

export interface PreKeyBundle {
  registrationId: number;
  identityKey: string;
  signedPreKey: { keyId: number; publicKey: string; signature: string };
  oneTimePreKey?: { keyId: number; publicKey: string };
}

export function useCrypto() {
  // @faridguzman91: Returns our own prekey bundle to share with the key server on registration
  async function generatePreKeyBundle(): Promise<PreKeyBundle> {
    return invoke<PreKeyBundle>("generate_prekey_bundle");
  }

  async function encryptMessage(
    recipientId: string,
    plaintext: string
  ): Promise<{ ciphertext: string; messageType: number }> {
    return invoke("encrypt_message", { recipientId, plaintext });
  }

  async function decryptMessage(
    senderId: string,
    ciphertext: string,
    messageType: number
  ): Promise<string> {
    return invoke("decrypt_message", { senderId, ciphertext, messageType });
  }

  // @faridguzman91: Runs X3DH + initialises a Double Ratchet session for recipientId
  async function initSession(recipientId: string, bundle: PreKeyBundle): Promise<void> {
    return invoke("init_session", { recipientId, bundle });
  }

  return { generatePreKeyBundle, encryptMessage, decryptMessage, initSession };
}
