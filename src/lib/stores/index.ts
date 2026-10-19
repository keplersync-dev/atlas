import { api } from "$lib/api";
import { instances } from "./instances.svelte";
import { social } from "./social.svelte";

export { instances, social };

/**
 * Loads every store and keeps them fresh from live events.
 * Call once, from the app shell. Returns a cleanup function.
 */
export function connectStores() {
  instances.load();
  social.load();

  return api.subscribe((e) => {
    switch (e.type) {
      case "instance.progress": {
        // Progress carries the new data, so patch in place instead of refetching.
        const i = instances.byId(e.instanceId);
        if (i) Object.assign(i, { status: e.status, progress: e.progress });
        break;
      }
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
      case "friends.changed":
      case "groups.changed":
      case "session.changed":
      case "connection.changed":
        social.load();
        break;
    }
  });
}
