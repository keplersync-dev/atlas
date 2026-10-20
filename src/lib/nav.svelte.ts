import type { Id } from "$lib/types";

/** Every screen the main area can show. Add a variant here when you add a page. */
export type View =
  | { page: "home" }
  | { page: "shared" }
  | { page: "friends" }
  | { page: "instance"; id: Id }
  | { page: "review"; id: Id };

const MAX_HISTORY = 50;

// The current page plus the pages you came through, like a browser's back stack.
const state = $state<{ view: View; history: View[] }>({
  view: { page: "home" },
  history: [],
});

/** Read-only view of navigation for components. Change it with go() / back(). */
export const nav = {
  get view() {
    return state.view;
  },
  get canGoBack() {
    return state.history.length > 0;
  },
};

const idOf = (v: View) => ("id" in v ? v.id : null);
const same = (a: View, b: View) => a.page === b.page && idOf(a) === idOf(b);

/**
 * Show a page. The page you leave is remembered for back().
 * `replace: true` swaps the current page without remembering it — for redirects
 * like "update finished → back to the instance", where Back shouldn't return to
 * a page that no longer makes sense.
 */
export function go(view: View, options: { replace?: boolean } = {}) {
  if (same(view, state.view)) return;
  if (!options.replace) {
    state.history.push(state.view);
    if (state.history.length > MAX_HISTORY) state.history.shift();
  }
  state.view = view;
}

/** Return to the previous page, if there is one. */
export function back() {
  let previous = state.history.pop();
  // Skip entries that are the page we're already on (left behind by replace/forget).
  while (previous && same(previous, state.view)) previous = state.history.pop();
  if (previous) state.view = previous;
}

/** Remove an instance's pages from history (it was untracked), so Back never lands on a dead page. */
export function forgetInstance(id: Id) {
  state.history = state.history
    .filter((v) => idOf(v) !== id)
    // removing entries can leave the same page twice in a row: keep one
    .filter((v, i, list) => i === 0 || !same(v, list[i - 1]));
}

export function isActive(view: View) {
  const current = state.view;
  // An instance's review page still highlights that instance in the sidebar.
  if (
    view.page === "instance" &&
    (current.page === "instance" || current.page === "review")
  ) {
    return current.id === view.id;
  }
  return current.page === view.page;
}
