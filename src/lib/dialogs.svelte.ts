import type { Id, OpenedLink } from "$lib/types";

/** Every app-wide dialog and the data it needs. Add a variant here when you add a dialog. */
export type ActiveDialog =
  | { kind: "share"; instanceId: Id }
  | { kind: "invite"; instanceId: Id }
  | { kind: "publish"; instanceId: Id }
  | { kind: "accept"; sharedInstanceId: Id }
  | { kind: "track" }
  | { kind: "account" }
  | { kind: "link"; link: OpenedLink }
  | { kind: "report"; about: ReportAbout };

/** What the report dialog is about: an uploaded file is found by its content row. */
export type ReportAbout =
  | { kind: "file"; instanceId: Id; contentId: Id; name: string }
  | { kind: "shared_instance"; id: Id; name: string }
  | { kind: "user"; id: Id; name: string };

const state = $state<{ active: ActiveDialog | null }>({ active: null });

export const dialogs = {
  get active() {
    return state.active;
  },
};

/** Open a dialog from anywhere: openDialog({ kind: "publish", instanceId }). */
export function openDialog(dialog: ActiveDialog) {
  state.active = dialog;
}

export function closeDialog() {
  state.active = null;
}
