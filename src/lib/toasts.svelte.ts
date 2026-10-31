// Errors from actions in long lists (a mod's sharing, a member's role, a backup) show
// at the bottom of the window, so they're seen however far down the list you are.
// Notices (something needs the player) show there too while Atlas is on screen.

import type { Notice } from "$lib/types";

type Toast =
  | { id: number; kind: "error"; text: string }
  | { id: number; kind: "notice"; notice: Notice };

/** How long an error stays up. */
const SHOW_MS = 8000;
/** A notice asks for something, so it stays longer. */
const NOTICE_MS = 15000;

const state = $state<{ items: Toast[] }>({ items: [] });
let next = 0;

export const toasts = {
  get items() {
    return state.items;
  },
};

export function showError(text: string) {
  // The same error twice in a row stays one toast.
  const id = ++next;
  state.items = [
    ...state.items.filter((t) => t.kind !== "error" || t.text !== text),
    { id, kind: "error" as const, text },
  ].slice(-3);
  setTimeout(() => dismissToast(id), SHOW_MS);
}

export function showNotice(notice: Notice) {
  const id = ++next;
  state.items = [...state.items, { id, kind: "notice" as const, notice }].slice(
    -3,
  );
  setTimeout(() => dismissToast(id), NOTICE_MS);
}

export function dismissToast(id: number) {
  state.items = state.items.filter((t) => t.id !== id);
}
