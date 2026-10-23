// Import `api` from "$lib/api" everywhere. Never import tauri.ts directly.

export { tauriApi as api } from "./tauri";
export { ApiError, type AtlasApi } from "./api";
