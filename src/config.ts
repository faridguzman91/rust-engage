// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2024-2026 Farid Guzman <https://github.com/faridguzman91>

// @faridguzman91: Central server URL config — override VITE_SERVER_URL in .env.local
// to point at a remote server. WebSocket URL is derived automatically.
export const SERVER_BASE = import.meta.env.VITE_SERVER_URL ?? "http://localhost:3000";
export const SERVER_WS = SERVER_BASE.replace(/^http/, "ws");
