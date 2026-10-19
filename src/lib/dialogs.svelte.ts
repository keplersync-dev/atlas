import type { Id } from "$lib/types";

/** Every app-wide dialog and the data it needs. Add a variant here when you add a dialog. */
export type ActiveDialog =
  | { kind: "share"; instanceId: Id }
  | { kind: "invite"; instanceId: Id }
  | { kind: "publish"; instanceId: Id }
  | { kind: "accept"; sharedInstanceId: Id }
  | { kind: "track" }
  | { kind: "account" };

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
