/** What a confirmation asks. */
export type ConfirmOptions = {
  title: string;
  message?: string;
  /** The button that goes ahead: "Stop tracking", "Delete account"… */
  confirmLabel: string;
  /** Red button, Cancel focused: for actions that lose something. */
  danger?: boolean;
  /** The user types this to enable the button, for things that can't be undone. */
  typeToConfirm?: string;
};

/** A confirmation that may have a second way to go ahead, a bigger one: "Stop sharing too". */
export type ChoiceOptions = ConfirmOptions & { alternativeLabel?: string };

export type Choice = "confirm" | "alternative";

type Open = ChoiceOptions & {
  answer: (choice: Choice | null) => void;
};

const state = $state<{ open: Open | null }>({ open: null });

export const confirmation = {
  get open() {
    return state.open;
  },
};

function ask(options: ChoiceOptions) {
  state.open?.answer(null);
  return new Promise<Choice | null>((resolve) => {
    state.open = {
      ...options,
      answer: (choice) => {
        state.open = null;
        resolve(choice);
      },
    };
  });
}

/**
 * Ask before doing something. Resolves true when the user goes ahead. Shows above any
 * open dialog, so it works from inside one too:
 *
 *   if (!(await confirmAction({ title: "Leave SMP?", confirmLabel: "Leave", danger: true }))) return;
 */
export async function confirmAction(options: ConfirmOptions): Promise<boolean> {
  return (await ask(options)) === "confirm";
}

/**
 * Like confirmAction, plus a second button when `alternativeLabel` is set. Resolves to the
 * button pressed, or null on Cancel.
 */
export function chooseAction(options: ChoiceOptions): Promise<Choice | null> {
  return ask(options);
}
