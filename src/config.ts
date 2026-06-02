export const SERVER_BASE = import.meta.env.VITE_SERVER_URL ?? "http://localhost:3000";
export const SERVER_WS = SERVER_BASE.replace(/^http/, "ws");
