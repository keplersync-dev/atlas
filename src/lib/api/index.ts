// Import `api` from "$lib/api" everywhere. Never import mock.ts or tauri.ts directly.
// Defaults to the mock; set VITE_ATLAS_API=tauri in .env.local once Rust commands exist.

import type { AtlasApi } from "./api";
import { mockApi } from "./mock";
import { tauriApi } from "./tauri";

export const api: AtlasApi =
  import.meta.env.VITE_ATLAS_API === "tauri" ? tauriApi : mockApi;

export { ApiError, type AtlasApi } from "./api";
