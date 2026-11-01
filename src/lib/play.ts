import { api } from "$lib/api";
import { showDone, showError } from "$lib/toasts.svelte";
import type { InstanceSummary } from "$lib/types";

/** Whether Play can do anything: a launcher instance with its folder, not already running. */
export const canPlay = (i: InstanceSummary) =>
  i.launcher.kind !== "folder" && i.status === "ready" && !i.isRunning;

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
