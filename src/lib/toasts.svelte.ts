// Errors from actions in long lists (a mod's sharing, a member's role, a backup) show
// at the bottom of the window, so they're seen however far down the list you are.

type Toast = { id: number; text: string };

/** How long an error stays up. */
const SHOW_MS = 8000;

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
    ...state.items.filter((t) => t.text !== text),
    { id, text },
  ].slice(-3);
  setTimeout(() => dismissToast(id), SHOW_MS);
}

export function dismissToast(id: number) {
  state.items = state.items.filter((t) => t.id !== id);
}
