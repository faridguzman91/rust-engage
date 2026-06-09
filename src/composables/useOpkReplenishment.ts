// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

// @faridguzman91: OPK (One-Time PreKey) replenishment composable.
//
// One-time prekeys provide forward secrecy for the first message in an X3DH session.
// Each contact that initiates a session with us consumes one OPK from the server pool.
// When the pool drops below LOW_WATERMARK (10), we generate a fresh batch of 100,
// store the private halves in local SQLite, and upload the public halves to the server.
//
// Triggered silently (non-blocking) from two places:
//   - useWebSocket.ts → onopen (every successful WS connect)
//   - contacts.ts → ensureSession (after each new outbound X3DH session)
import { invoke } from "@tauri-apps/api/core";
import { useAuthStore } from "../stores/auth";
import { useServerApi } from "./useServerApi";

// @faridguzman: Threshold and batch size are enforced on the Rust side via
// get_opk_status / generate_and_store_opks — no TS constants needed here.

// @faridguzman91: Module-level debounce flag prevents concurrent replenishment runs
let replenishing = false;

export function useOpkReplenishment() {
  const auth = useAuthStore();
  const api = useServerApi();

  async function checkAndReplenish(): Promise<void> {
    if (replenishing || !auth.profile) return;

    try {
      // 1. Ask the server how many OPKs it still has for us
      const { remaining } = await api.fetchOpkCount(auth.profile.userId);

      // 2. Check against watermark via Rust (keeps the threshold logic server-agnostic)
      const { needs_replenishment } = await invoke<{ server_remaining: number; needs_replenishment: boolean }>(
        "get_opk_status",
        { serverRemaining: remaining }
      );

      if (!needs_replenishment) return;

      replenishing = true;

      // 3. Generate fresh batch locally — private halves stored in SQLite, never leave device
      const { public_keys } = await invoke<{
        public_keys: { keyId: number; publicKey: string }[];
        count: number;
      }>("generate_and_store_opks");

      // 4. Upload only the public halves to the server
      await api.uploadPreKeys(auth.profile.userId, public_keys);

      console.debug(`[OPK] Replenished ${public_keys.length} keys (was ${remaining} remaining)`);
    } catch (e) {
      // @faridguzman91: Non-fatal — will retry on the next WS connect or session init
      console.warn("[OPK] Replenishment failed:", e);
    } finally {
      replenishing = false;
    }
  }

  return { checkAndReplenish };
}
