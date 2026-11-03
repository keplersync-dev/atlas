import { api } from "$lib/api";
import type { Id, InstanceSummary, InstanceWork } from "$lib/types";

/** The tracked instances, shared by every component that shows them. */
class InstancesStore {
  list = $state<InstanceSummary[]>([]);
  loaded = $state(false);
  error = $state<string | null>(null);
  /** Why the last update/restore of an instance failed, until dismissed or retried. */
  failures = $state<Record<Id, string>>({});
  /** Work under way outside an update: reading files, uploading, saving a version. */
  work = $state<Record<Id, InstanceWork>>({});
  #version = 0;

  async load() {
    // A slow older reply mustn't overwrite a newer one.
    const v = ++this.#version;
    try {
      const list = await api.listInstances();
      if (v !== this.#version) return;
      this.list = list;
      this.error = null;
    } catch (e) {
      if (v === this.#version) this.error = (e as Error).message;
    } finally {
      this.loaded = true;
    }
  }

  byId(id: Id) {
    return this.list.find((i) => i.id === id);
  }

  dismissFailure(id: Id) {
    delete this.failures[id];
  }
}

export const instances = new InstancesStore();
