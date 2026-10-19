import type { Id } from "$lib/types";

/** Every screen the main area can show. Add a variant here when you add a page. */
export type View =
  | { page: "home" }
  | { page: "shared" }
  | { page: "friends" }
  | { page: "instance"; id: Id }
  | { page: "review"; id: Id };

export const nav = $state<{ view: View }>({ view: { page: "home" } });

export function go(view: View) {
  nav.view = view;
}

export function isActive(view: View) {
  const current = nav.view;
  // An instance's review page still highlights that instance in the sidebar.
  if (
    view.page === "instance" &&
    (current.page === "instance" || current.page === "review")
  ) {
    return current.id === view.id;
  }
  return current.page === view.page;
}
