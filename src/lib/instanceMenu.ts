import { api } from "$lib/api";
import { chooseAction, confirmAction, type Choice } from "$lib/confirm.svelte";
import { openDialog } from "$lib/dialogs.svelte";
import { plural } from "$lib/format";
import { forgetInstance, go, nav } from "$lib/nav.svelte";
import { isMac } from "$lib/platform";
import { instances, social } from "$lib/stores";
import { showError } from "$lib/toasts.svelte";
import type { InstanceSummary } from "$lib/types";
import type { MenuItem } from "../components/MenuList.svelte";

type Run = (action: () => Promise<unknown>) => unknown;

/** Runs a menu action outside the instance page: errors show as a toast. */
const toastRun: Run = (action) =>
  action()
    .then(() => instances.load())
    .catch((e) => showError((e as Error).message));

/** The shared pack this is linked to, if any (dev-folder links aren't on the server). */
const sharedIdOf = (i: InstanceSummary) =>
  i.sync && !i.sync.sharedInstanceId.startsWith("dev:")
    ? i.sync.sharedInstanceId
    : null;

/** An instance's menu: the page's ⋯, and right-click on Home cards and the sidebar.
 *  `onPage`: the page has its own Share button and is already open. `run`: the page
 *  shows errors and reloads itself. */
export function instanceItems(
  i: InstanceSummary,
  { onPage = false, run = toastRun }: { onPage?: boolean; run?: Run } = {},
): MenuItem[] {
  const s = i.sync;
  const owner = s?.owner?.displayName ?? "the owner";
  return [
    ...(onPage
      ? []
      : [
          {
            label: "Open",
            icon: "grid",
            onselect: () => go({ page: "instance", id: i.id }),
          },
        ]),
    ...(i.status === "missing"
      ? []
      : [
          {
            label: isMac ? "Show in Finder" : "Show in File Explorer",
            icon: "folders",
            onselect: () => run(() => api.openInstanceFolder(i.id)),
          },
          {
            label: "Rescan folder",
            icon: "refresh",
            onselect: () => run(() => api.rescanInstance(i.id)),
          },
          {
            label: "Export as .mrpack…",
            icon: "download",
            onselect: () => run(() => api.exportMrpack(i.id)),
          },
        ]),
    "divider",
    ...(onPage ? [] : shareItem(i)),
    "divider",
    ...(s && s.role !== "owner"
      ? [
          {
            label: "Stop getting updates",
            hint: `Leave ${owner}'s pack. Your files stay as they are.`,
            danger: true,
            onselect: async () => {
              const ok = await confirmAction({
                title: `Stop getting updates for ${i.name}?`,
                message: `You leave ${owner}'s pack. Your copy stays here and keeps working, but won't change when they send updates. They'd have to invite you again.`,
                confirmLabel: "Stop updates",
                danger: true,
              });
              if (ok) run(() => api.leaveSharedInstance(s.sharedInstanceId));
            },
          },
        ]
      : []),
    ...(s && s.role === "owner"
      ? [
          {
            label: "Stop sharing",
            hint: "Friends keep their copy but get no more updates.",
            danger: true,
            onselect: async () => {
              const ok = await confirmAction({
                title: `Stop sharing ${i.name}?`,
                message:
                  "Nobody gets updates anymore, you included. Everyone keeps their copy as a normal instance.",
                confirmLabel: "Stop sharing",
                danger: true,
              });
              if (ok) run(() => api.archiveSharedInstance(s.sharedInstanceId));
            },
          },
        ]
      : []),
    {
      label: "Remove from Atlas",
      hint: `Atlas forgets it. ${staysWhere(i)}.`,
      danger: true,
      onselect: () => removeFromAtlas(i),
    },
    ...(s && s.role !== "owner"
      ? [
          "divider" as const,
          {
            label: "Report…",
            onselect: () =>
              openDialog({
                kind: "report",
                about: {
                  kind: "shared_instance",
                  id: s.sharedInstanceId,
                  name: i.name,
                },
              }),
          },
        ]
      : []),
  ];
}

/** The main sharing action, whichever applies: share, send an update, or invite. */
function shareItem(i: InstanceSummary): MenuItem[] {
  const s = i.sync;
  if (!s && !i.sharedElsewhere && i.status === "ready")
    return [
      {
        label: "Share…",
        icon: "share",
        onselect: () => openDialog({ kind: "share", instanceId: i.id }),
      },
    ];
  if (s?.state === "local_changes" && s.role !== "viewer")
    return [
      {
        label: "Send update…",
        icon: "share",
        onselect: () => openDialog({ kind: "publish", instanceId: i.id }),
      },
    ];
  if (s && s.role !== "viewer" && s.state !== "frozen")
    return [
      {
        label: "Invite…",
        icon: "users",
        onselect: () => openDialog({ kind: "invite", instanceId: i.id }),
      },
    ];
  return [];
}

const stopTracking = { confirmLabel: "Remove", danger: true };

/** "It stays in Prism Launcher", or for a folder added by hand, where it is. */
const staysWhere = (i: InstanceSummary) =>
  i.launcher.kind === "folder"
    ? "Its folder stays on your PC"
    : `It stays in ${i.launcher.name}`;

/** Remove an instance from Atlas after a confirm that says what happens to its pack. */
export async function removeFromAtlas(i: InstanceSummary) {
  const sharedId = sharedIdOf(i);
  const forget = `Atlas forgets it and deletes its backups. ${staysWhere(i)}, untouched.`;
  let choice: Choice | null;
  if (!sharedId) {
    choice = await chooseAction({
      ...stopTracking,
      title: `Remove ${i.name} from Atlas?`,
      message: `${forget} You can add it again later.`,
    });
  } else if (i.sync?.role === "owner") {
    // Stopping tracking here never ends the sharing: you may have it on another PC.
    const shared = social.shared.find((s) => s.id === sharedId);
    const people =
      shared?.members.filter((m) => m.role !== "owner" && m.status === "active")
        .length ?? 0;
    choice = await chooseAction({
      ...stopTracking,
      title: `Remove ${i.name} from Atlas?`,
      message: `${forget} It stays shared${people ? ` with ${plural(people, "person", "people")}` : ""}, and you can add it back from Shared to send updates again.`,
      alternativeLabel: "Stop sharing too",
    });
    if (
      choice === "alternative" &&
      !(await confirmAction({
        title: `Stop sharing ${i.name}?`,
        message:
          "Nobody gets updates anymore. Everyone keeps their copy as a normal instance.",
        confirmLabel: "Stop sharing",
        danger: true,
      }))
    )
      return;
  } else {
    choice = await chooseAction({
      ...stopTracking,
      title: `Remove ${i.name} from Atlas?`,
      message: `${forget} You still get its updates, so you can add it back from Shared.`,
      alternativeLabel: "Stop updates too",
    });
  }
  if (!choice) return;
  try {
    if (choice === "alternative" && sharedId)
      await (i.sync?.role === "owner"
        ? api.archiveSharedInstance(sharedId)
        : api.leaveSharedInstance(sharedId));
    await api.untrackInstance(i.id);
  } catch (e) {
    showError((e as Error).message);
    await instances.load();
    return;
  }
  await social.load();
  await instances.load();
  // Its page no longer exists: leave it, and drop it from history.
  forgetInstance(i.id);
  const v = nav.view;
  if ((v.page === "instance" || v.page === "review") && v.id === i.id)
    go({ page: "home" }, { replace: true });
}
