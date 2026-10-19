import { api } from "$lib/api";
import type { Id, InstanceSummary } from "$lib/types";

/**
 * The tracked instances, shared by every component that shows them.
 *
 * Fields declared with $state are reactive: any component that reads
 * `instances.list` re-renders when it changes. $derived fields recompute
 * automatically from the $state they read.
 */
class InstancesStore {
  list = $state<InstanceSummary[]>([]);
  loaded = $state(false);
  error = $state<string | null>(null);

  /** Instances with something waiting on the user: an update to review or changes to publish. */
  needingAttention = $derived(
    this.list.filter(
      (i) =>
        i.sync?.state === "update_available" ||
        i.sync?.state === "conflict" ||
        i.sync?.state === "local_changes",
    ),
  );

  /** Most recently played first; never-played ones at the end. */
  recentlyPlayed = $derived(
    this.list
      .filter((i) => i.status !== "missing")
      .toSorted((a, b) =>
        (b.lastPlayedAt ?? "").localeCompare(a.lastPlayedAt ?? ""),
      ),
  );

  async load() {
    try {
      this.list = await api.listInstances();
      this.error = null;
    } catch (e) {
      this.error = (e as Error).message;
    } finally {
      this.loaded = true;
    }
  }

  byId(id: Id) {
    return this.list.find((i) => i.id === id);
  }
}

export const instances = new InstancesStore();
