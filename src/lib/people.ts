import { confirmAction } from "$lib/confirm.svelte";
import { openDialog } from "$lib/dialogs.svelte";
import { social } from "$lib/stores";
import { showDone, showError } from "$lib/toasts.svelte";
import type { PublicUser } from "$lib/types";
import type { MenuItem } from "../components/MenuList.svelte";

/** What you can do about another person wherever they're shown (Friends, the friends
 *  column on Home, group members): report them, unfriend them, block them. */
export function personItems(user: PublicUser): MenuItem[] {
  if (user.id === social.session?.user?.id) return [];
  const friend = social.friends.some((f) => f.user.id === user.id);
  return [
    {
      label: "Report…",
      icon: "flag",
      onselect: () =>
        openDialog({
          kind: "report",
          about: { kind: "user", id: user.id, name: user.displayName },
        }),
    },
    "divider",
    ...(friend
      ? [
          {
            label: "Remove friend",
            danger: true,
            onselect: () => unfriend(user, false),
          },
        ]
      : []),
    { label: "Block", danger: true, onselect: () => unfriend(user, true) },
  ];
}

async function unfriend(user: PublicUser, block: boolean) {
  const name = user.displayName;
  const friend = social.friends.some((f) => f.user.id === user.id);
  const ok = await confirmAction(
    block
      ? {
          title: `Block ${name}?`,
          message: `${friend ? `${name} stops being your friend and` : `${name}`} can't send you friend requests. They aren't told, and groups you're both in stay as they are. Unblock them any time in Settings.`,
          confirmLabel: "Block",
          danger: true,
        }
      : {
          title: `Remove ${name}?`,
          message: "You can add each other again later with a friend code.",
          confirmLabel: "Remove friend",
          danger: true,
        },
  );
  if (!ok) return;
  try {
    await social.removeFriend(user.id, block);
    if (block) showDone(`Blocked ${name}. Unblock them in Settings.`);
  } catch (e) {
    showError((e as Error).message);
  }
}
