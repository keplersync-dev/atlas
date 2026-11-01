import { api } from "$lib/api";
import { openDialog } from "$lib/dialogs.svelte";
import { go } from "$lib/nav.svelte";
import { canPlay, play } from "$lib/play";
import { showError } from "$lib/toasts.svelte";
import type { InstanceSummary } from "$lib/types";
import type { MenuItem } from "../components/MenuList.svelte";

/** The right-click menu of an instance on Home or in the sidebar: open it, play it,
 *  see its files, and its main sharing action. Removing it stays on its page, where its
 *  confirm can say what happens to a shared one. */
export function instanceItems(i: InstanceSummary): MenuItem[] {
  const s = i.sync;
  return [
    {
      label: "Open",
      icon: "grid",
      onselect: () => go({ page: "instance", id: i.id }),
    },
    ...(canPlay(i)
      ? [{ label: "Play", icon: "play", onselect: () => play(i) }]
      : []),
    ...(i.status !== "missing"
      ? [
          {
            label: "Show in File Explorer",
            icon: "folders",
            onselect: () =>
              api
                .openInstanceFolder(i.id)
                .catch((e) => showError((e as Error).message)),
          },
        ]
      : []),
    "divider",
    ...(!s && !i.sharedElsewhere && i.status === "ready"
      ? [
          {
            label: "Share…",
            icon: "share",
            onselect: () => openDialog({ kind: "share", instanceId: i.id }),
          },
        ]
      : s?.state === "local_changes" && s.role !== "viewer"
        ? [
            {
              label: "Send update…",
              icon: "share",
              onselect: () => openDialog({ kind: "publish", instanceId: i.id }),
            },
          ]
        : s && s.role !== "viewer" && s.state !== "frozen"
          ? [
              {
                label: "Invite…",
                icon: "users",
                onselect: () =>
                  openDialog({ kind: "invite", instanceId: i.id }),
              },
            ]
          : []),
  ];
}
