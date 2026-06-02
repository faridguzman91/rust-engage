import { invoke } from "@tauri-apps/api/core";

export interface PreKeyBundle {
  registrationId: number;
  identityKey: string;
  signedPreKey: { keyId: number; publicKey: string; signature: string };
  oneTimePreKey?: { keyId: number; publicKey: string };
}

export function useCrypto() {
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

  async function initSession(recipientId: string, bundle: PreKeyBundle): Promise<void> {
    return invoke("init_session", { recipientId, bundle });
  }

  return { generatePreKeyBundle, encryptMessage, decryptMessage, initSession };
}
