// @faridguzman91: Central server URL config — override VITE_SERVER_URL in .env.local
// to point at a remote server. WebSocket URL is derived automatically.
export const SERVER_BASE = import.meta.env.VITE_SERVER_URL ?? "http://localhost:3000";
export const SERVER_WS = SERVER_BASE.replace(/^http/, "ws");
