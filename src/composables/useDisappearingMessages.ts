// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

// @faridguzman91: Composable for disappearing message timers.
//
// Responsibilities:
//   1. scheduleExpiry()   — sets a JS timeout that removes a message from the Pinia store
//                           when its expires_at timestamp is reached
//   2. startSweep()       — on app start, calls the Rust sweep command to clear
//                           any DB rows that expired while the app was closed,
//                           then re-schedules live timers for messages that are
//                           still active
//   3. formatTimer()      — converts raw seconds to a human-readable label
import { invoke } from "@tauri-apps/api/core";
import { useMessagesStore } from "../stores/messages";
import type { Message } from "../stores/messages";

// @faridguzman91: Map of messageId → timeout handle so we can cancel if needed
const timers = new Map<string, ReturnType<typeof setTimeout>>();

export const TIMER_OPTIONS: { label: string; value: number }[] = [
  { label: "Off",    value: 0       },
  { label: "30s",    value: 30      },
  { label: "5 min",  value: 300     },
  { label: "1 hour", value: 3600    },
  { label: "8 hours",value: 28800   },
  { label: "1 week", value: 604800  },
];

export function formatTimer(secs: number): string {
  if (secs === 0)       return "Off";
  if (secs < 60)        return `${secs}s`;
  if (secs < 3600)      return `${secs / 60}m`;
  if (secs < 86400)     return `${secs / 3600}h`;
  return `${secs / 86400}d`;
}

export function useDisappearingMessages() {
  const messagesStore = useMessagesStore();

  // @faridguzman91: Schedule a JS timeout that removes a message from the store
  // when its expiry time arrives. Cancels any existing timer for the same message.
  function scheduleExpiry(msg: Message) {
    if (!msg.expiresAt) return;

    const remaining = msg.expiresAt - Date.now();
    if (remaining <= 0) {
      // Already expired — remove immediately
      messagesStore.remove(msg.id, msg.conversationId);
      return;
    }

    if (timers.has(msg.id)) {
      clearTimeout(timers.get(msg.id)!);
    }

    const handle = setTimeout(() => {
      messagesStore.remove(msg.id, msg.conversationId);
      timers.delete(msg.id);
    }, remaining);

    timers.set(msg.id, handle);
  }

  // @faridguzman91: Run the Rust sweep on startup (clears rows that expired
  // while the app was closed) then re-schedule timers for surviving messages.
  async function startSweep(activeMessages: Message[]) {
    try {
      const { deleted } = await invoke<{ deleted: number }>("sweep_expired_messages");
      if (deleted > 0) {
        console.debug(`[disappear] Swept ${deleted} expired messages on startup`);
      }
    } catch { /* non-fatal */ }

    // Re-schedule live timers for all messages that still have a future expiry
    for (const msg of activeMessages) {
      if (msg.expiresAt && msg.expiresAt > Date.now()) {
        scheduleExpiry(msg);
      }
    }
  }

  return { scheduleExpiry, startSweep };
}
