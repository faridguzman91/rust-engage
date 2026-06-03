/**
 * useOpkReplenishment
 *
 * Checks the server's one-time prekey count for the current user and
 * silently uploads a fresh batch when the pool falls below LOW_WATERMARK (10).
 *
 * Call `checkAndReplenish()`:
 *   - On app start (after WS connects)
 *   - After establishing a new outbound X3DH session (contact claims an OPK)
 */
import { invoke } from "@tauri-apps/api/core";
import { useAuthStore } from "../stores/auth";
import { useServerApi } from "./useServerApi";

const LOW_WATERMARK = 10;

// Debounce flag — avoid concurrent replenishment runs
let replenishing = false;

export function useOpkReplenishment() {
  const auth = useAuthStore();
  const api = useServerApi();

  async function checkAndReplenish(): Promise<void> {
    if (replenishing || !auth.profile) return;

    try {
      // 1. Ask the server how many OPKs it still has for us
      const { remaining } = await api.fetchOpkCount(auth.profile.userId);

      // 2. Check threshold
      const { needs_replenishment } = await invoke<{ server_remaining: number; needs_replenishment: boolean }>(
        "get_opk_status",
        { serverRemaining: remaining }
      );

      if (!needs_replenishment) return;

      replenishing = true;

      // 3. Generate fresh batch locally (stores private halves in SQLite)
      const { public_keys } = await invoke<{
        public_keys: { keyId: number; publicKey: string }[];
        count: number;
      }>("generate_and_store_opks");

      // 4. Upload public halves to server
      await api.uploadPreKeys(auth.profile.userId, public_keys);

      console.debug(`[OPK] Replenished ${public_keys.length} one-time prekeys (was ${remaining} remaining)`);
    } catch (e) {
      // Non-fatal — will retry on next trigger
      console.warn("[OPK] Replenishment failed:", e);
    } finally {
      replenishing = false;
    }
  }

  return { checkAndReplenish };
}
