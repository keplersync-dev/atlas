import { api } from "$lib/api";
import type { AvailableUpdate } from "$lib/types";

const CHECK_EVERY_MS = 6 * 60 * 60 * 1000;

/** A newer Atlas: found on the release page, or demanded by the server. */
class UpdatesStore {
  available = $state<AvailableUpdate | null>(null);
  /** The server refused this version: syncing stops until it's updated. */
  required = $state(false);
  installing = $state(false);
  /** 0–1 while downloading, null when the size isn't known yet. */
  progress = $state<number | null>(null);
  error = $state<string | null>(null);

  /** Check now and every few hours. Returns a cleanup function. */
  start() {
    this.check();
    const timer = setInterval(() => this.check(), CHECK_EVERY_MS);
    return () => clearInterval(timer);
  }

  async check() {
    try {
      this.available = await api.checkForUpdate();
    } catch {
      // Offline, or GitHub is down: the next check tries again.
    }
  }

  /** The server said this version is too old: look for the update straight away. */
  require() {
    if (this.required) return;
    this.required = true;
    this.check();
  }

  setProgress(downloaded: number, total: number | null) {
    this.progress = total ? Math.min(1, downloaded / total) : null;
  }

  async install() {
    this.installing = true;
    this.error = null;
    this.progress = null;
    try {
      // Atlas quits and restarts into the new version, so this only returns on failure.
      await api.installUpdate();
    } catch (err) {
      this.error = (err as Error).message;
    } finally {
      this.installing = false;
    }
  }
}

export const updates = new UpdatesStore();
