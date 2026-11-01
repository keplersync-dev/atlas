import { api } from "$lib/api";
import { showDone, showError } from "$lib/toasts.svelte";
import type { InstanceSummary } from "$lib/types";

/** A friend's pack whose first install never finished (it was cancelled, say): linked,
 *  but none of its files are in yet. It needs finishing, not "an update". */
export const unfinished = (i: InstanceSummary) => i.sync?.syncedRevision === 0;

/** Whether Play can do anything: a launcher instance with its folder and its files, not
 *  already running. */
export const canPlay = (i: InstanceSummary) =>
  i.launcher.kind !== "folder" &&
  i.status === "ready" &&
  !i.isRunning &&
  !unfinished(i);

/** Start the game, or open the launcher where it can't be started from outside. */
export async function play(i: InstanceSummary) {
  try {
    const started = await api.playInstance(i.id);
    showDone(
      started
        ? `Starting ${i.name} in ${i.launcher.name}.`
        : `${i.launcher.name} is opening. Press Play on ${i.name} there.`,
    );
  } catch (e) {
    showError((e as Error).message);
  }
}
