import { api } from "$lib/api";
import { openDialog } from "$lib/dialogs.svelte";
import { instances } from "./instances.svelte";
import { social } from "./social.svelte";
import { updates } from "./updates.svelte";

export { instances, social, updates };

/**
 * Loads every store and keeps them fresh from live events.
 * Call once, from the app shell. Returns a cleanup function.
 */
export function connectStores() {
  instances.load();
  social.load();
  const stopChecking = updates.start();
  // A link may have started Atlas before the page was ready to hear about it.
  showOpenedLink();

  const unsubscribe = api.subscribe((e) => {
    switch (e.type) {
      case "update.progress":
        updates.setProgress(e.downloaded, e.total);
        break;
      case "update.required":
        updates.require();
        break;
      case "instance.progress": {
        // Progress carries the new data, so patch in place instead of refetching.
        const i = instances.byId(e.instanceId);
        if (i) Object.assign(i, { status: e.status, progress: e.progress });
        // A new job starting clears the last failure.
        if (e.status === "syncing" && !e.progress)
          instances.dismissFailure(e.instanceId);
        break;
      }
      case "instance.work":
        if (e.work) instances.work[e.instanceId] = e.work;
        else delete instances.work[e.instanceId];
        break;
      case "instance.jobFailed":
        instances.failures[e.instanceId] = e.message;
        break;
      case "presence.changed": {
        const f = social.friends.find((x) => x.user.id === e.userId);
        if (f) f.presence = e.presence;
        break;
      }
      case "instance.changed":
      case "instance.running":
      case "launchers.changed":
        instances.load();
        break;
      case "shared.changed":
        instances.load();
        social.load();
        break;
      case "activity.new":
        social.activity.unshift(e.item);
        break;
      case "link.opened":
        showOpenedLink();
        break;
      case "friends.changed":
      case "groups.changed":
      case "session.changed":
      case "connection.changed":
        social.load();
        break;
    }
  });
  return () => {
    unsubscribe();
    stopChecking();
  };
}

/** Ask about a friend or group link Atlas was opened with, if one is waiting. */
async function showOpenedLink() {
  const link = await api.takeOpenedLink().catch(() => null);
  if (link) openDialog({ kind: "link", link });
}
