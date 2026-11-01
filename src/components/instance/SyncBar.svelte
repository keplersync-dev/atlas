<script lang="ts">
  import { api } from "$lib/api";
  import { plural, workLabel, workPercent } from "$lib/format";
  import { instances } from "$lib/stores";
  import type { InstanceSummary, SyncProgress } from "$lib/types";
  import Button from "../Button.svelte";
  import Icon from "../Icon.svelte";
  import ManualDownloads from "./ManualDownloads.svelte";

  // The one-line status strip under the instance header. Shows the single most
  // important thing about this folder right now, or nothing when all is well.
  let {
    instance,
    busy = false,
    onupdate,
    onrevert,
    onuntrack,
    onsetup,
  }: {
    instance: InstanceSummary;
    busy?: boolean;
    onupdate: () => void;
    onrevert: () => void;
    onuntrack: () => void;
    /** Shared packs only: add a fresh copy in place of the missing one. */
    onsetup?: () => void;
  } = $props();

  const STAGES: Record<SyncProgress["stage"], string> = {
    waiting_for_game_exit: "Waiting for you to close Minecraft…",
    backup: "Backing up files…",
    download: "Downloading…",
    waiting_for_manual_downloads: "Waiting for you to download files",
    apply: "Applying changes…",
  };

  const s = $derived(instance.sync);
  let cancelling = $state(false);
  // Once files are being changed, the job can't stop.
  const cancellable = $derived(
    instance.progress && instance.progress.stage !== "apply",
  );

  async function cancel() {
    cancelling = true;
    try {
      await api.cancelJob(instance.id);
    } catch {
      // It finished meanwhile.
    } finally {
      cancelling = false;
    }
  }
  // Waiting on the player: a progress bar would sit at 0%.
  const waiting = $derived(
    instance.progress?.stage === "waiting_for_game_exit" ||
      instance.progress?.stage === "waiting_for_manual_downloads",
  );
  // Reading files after tracking, or uploading while sharing.
  const work = $derived(instances.work[instance.id]);
</script>

{#snippet bar(
  tone: "warn" | "plain",
  icon: string,
  title: string,
  detail: string,
)}
  <div
    class="mt-5 flex items-center gap-3.5 rounded-lg py-3 pr-3.5 pl-4 {tone ===
    'warn'
      ? 'bg-(--warn)/10 [&>.ico]:text-(--warn)'
      : 'bg-(--bg-raise) [&>.ico]:text-(--text-faint)'}"
  >
    <span class="ico"><Icon name={icon} class="size-4.5" /></span>
    <div class="min-w-0 flex-1">
      <b class="font-semibold">{title}</b>
      <small class="block text-[13px] text-(--text-muted) select-text"
        >{detail}</small
      >
    </div>
    {#if tone === "warn" && (s?.state === "update_available" || s?.state === "conflict")}
      <Button variant="warn" disabled={busy} onclick={onupdate}
        >Review update</Button
      >
    {:else if instance.status === "missing"}
      {#if onsetup}
        <Button variant="ghost" onclick={onuntrack}>Remove from Atlas</Button>
        <Button variant="primary" onclick={onsetup}>Set it up again</Button>
      {:else}
        <Button onclick={onuntrack}>Remove from Atlas</Button>
      {/if}
    {:else if s?.state === "local_changes" && s.role === "viewer"}
      <Button disabled={busy} onclick={onrevert}>Undo changes</Button>
    {/if}
  </div>
{/snippet}

{#if instance.status === "missing"}
  {@render bar(
    "warn",
    "warn",
    "Atlas can't find this folder",
    s
      ? `It may have been deleted or moved in ${instance.launcher.name}. Set it up again to get a fresh copy of ${s.role === "owner" ? "what you shared" : `${s.owner?.displayName ?? "the owner"}'s pack`}. Expected at ${instance.gameDir}`
      : `It may have been deleted or moved in ${instance.launcher.name}. Expected at ${instance.gameDir}`,
  )}
{:else if instance.status === "syncing" && instance.progress}
  <div class="mt-5 rounded-lg bg-(--bg-raise) px-4 py-3">
    <div class="flex items-center justify-between gap-3 text-[13px]">
      <span class="flex-1">{STAGES[instance.progress.stage]}</span>
      {#if !waiting}
        <span class="text-(--text-muted) tabular-nums"
          >{instance.progress.percent}%</span
        >
      {/if}
      {#if cancellable}
        <Button size="sm" variant="ghost" disabled={cancelling} onclick={cancel}
          >Cancel</Button
        >
      {/if}
    </div>
    {#if instance.progress.stage === "waiting_for_manual_downloads"}
      <div class="mt-3 border-t border-(--line) pt-3">
        <ManualDownloads {instance} />
      </div>
    {:else if !waiting}
      <div class="mt-2 h-1 overflow-hidden rounded-full bg-(--line)">
        <div
          class="h-full bg-(--accent) transition-[width] duration-300"
          style="width: {instance.progress.percent}%"
        ></div>
      </div>
    {/if}
  </div>
{:else if work}
  {@const percent = workPercent(work)}
  <div class="mt-5 rounded-lg bg-(--bg-raise) px-4 py-3">
    <div class="flex justify-between text-[13px]">
      <span class="tabular-nums">{workLabel(work)}</span>
      {#if percent !== null}<span class="text-(--text-muted) tabular-nums"
          >{percent}%</span
        >{/if}
    </div>
    <div class="mt-2 h-1 overflow-hidden rounded-full bg-(--line)">
      <div
        class="h-full bg-(--accent) transition-[width] duration-300 {percent ===
        null
          ? 'w-full animate-pulse'
          : ''}"
        style={percent === null ? undefined : `width: ${percent}%`}
      ></div>
    </div>
  </div>
{:else if instance.isRunning}
  {@render bar(
    "warn",
    "clock",
    "Minecraft is running this instance",
    "Atlas won't change any files until you close the game.",
  )}
{:else if s?.state === "update_available" || s?.state === "conflict"}
  {@render bar(
    "warn",
    "download",
    s.role === "owner"
      ? "There's an update from an editor"
      : `${s.owner?.displayName ?? "The owner"} sent an update`,
    s.state === "conflict"
      ? `Updating undoes ${plural(s.localChangeCount, "change")} you made on this PC. Changed files are backed up first.`
      : "Changed files are backed up first.",
  )}
{:else if s?.state === "local_changes"}
  {@render bar(
    "plain",
    "refresh",
    `${plural(s.localChangeCount, "change")} on this PC only`,
    s.role === "viewer"
      ? `Friends don't get ${s.localChangeCount === 1 ? "it" : "them"}. Undo to match ${s.owner?.displayName ?? "the owner"}'s pack again.`
      : "Send an update so everyone has the same mods.",
  )}
{:else if s?.state === "frozen"}
  {@render bar(
    "plain",
    "shield",
    "No longer updated",
    "Its owner left and nobody took over. Your copy keeps working, but there won't be updates.",
  )}
{/if}
