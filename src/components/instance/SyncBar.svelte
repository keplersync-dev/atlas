<script lang="ts">
  import { plural, workLabel, workPercent } from "$lib/format";
  import { instances } from "$lib/stores";
  import type { InstanceSummary, SyncProgress } from "$lib/types";
  import Button from "../Button.svelte";
  import Icon from "../Icon.svelte";
  import CloseLauncherPrompt from "./CloseLauncherPrompt.svelte";

  // The one-line status strip under the instance header. Shows the single most
  // important thing about this folder right now, or nothing when all is well.
  let {
    instance,
    busy = false,
    onupdate,
    onrevert,
    onuntrack,
  }: {
    instance: InstanceSummary;
    busy?: boolean;
    onupdate: () => void;
    onrevert: () => void;
    onuntrack: () => void;
  } = $props();

  const STAGES: Record<SyncProgress["stage"], string> = {
    waiting_for_game_exit: "Waiting for you to close Minecraft…",
    waiting_for_launcher_exit: "Waiting for the launcher to close…",
    backup: "Backing up files…",
    download: "Downloading…",
    waiting_for_manual_downloads: "Waiting for manual downloads…",
    apply: "Applying changes…",
  };

  const s = $derived(instance.sync);
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
      <small class="block text-xs text-(--text-muted) select-text"
        >{detail}</small
      >
    </div>
    {#if tone === "warn" && (s?.state === "update_available" || s?.state === "conflict")}
      <Button variant="warn" disabled={busy} onclick={onupdate}
        >Review update</Button
      >
    {:else if instance.status === "missing"}
      <Button onclick={onuntrack}>Stop tracking</Button>
    {:else if s?.state === "local_changes" && s.role === "viewer"}
      <Button disabled={busy} onclick={onrevert}>Revert</Button>
    {/if}
  </div>
{/snippet}

{#if instance.status === "missing"}
  {@render bar(
    "warn",
    "warn",
    "Atlas can't find this folder",
    `It may have been deleted or moved in ${instance.launcher.name}. Expected at ${instance.gameDir}`,
  )}
{:else if instance.status === "syncing" && instance.progress}
  <div class="mt-5 rounded-lg bg-(--bg-raise) px-4 py-3">
    <div class="flex justify-between text-[12.5px]">
      <span>{STAGES[instance.progress.stage]}</span>
      <span class="text-(--text-muted) tabular-nums"
        >{instance.progress.percent}%</span
      >
    </div>
    <div class="mt-2 h-1 overflow-hidden rounded-full bg-(--line-2)">
      <div
        class="h-full bg-(--accent) transition-[width] duration-300"
        style="width: {instance.progress.percent}%"
      ></div>
    </div>
    {#if instance.progress.stage === "waiting_for_launcher_exit"}
      <div class="mt-3"><CloseLauncherPrompt {instance} /></div>
    {/if}
  </div>
{:else if work}
  {@const percent = workPercent(work)}
  <div class="mt-5 rounded-lg bg-(--bg-raise) px-4 py-3">
    <div class="flex justify-between text-[12.5px]">
      <span class="tabular-nums">{workLabel(work)}</span>
      {#if percent !== null}<span class="text-(--text-muted) tabular-nums"
          >{percent}%</span
        >{/if}
    </div>
    <div class="mt-2 h-1 overflow-hidden rounded-full bg-(--line-2)">
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
    `v${s.latestRevision} is available`,
    s.state === "conflict"
      ? `You're on v${s.syncedRevision}. Updating will undo ${plural(s.localChangeCount, "local change")}.`
      : `You're on v${s.syncedRevision}. Changed files are backed up first.`,
  )}
{:else if s?.state === "local_changes"}
  {@render bar(
    "plain",
    "refresh",
    `${plural(s.localChangeCount, "local change")} not published yet`,
    s.role === "viewer"
      ? `These only exist on this PC. Revert to match v${s.syncedRevision} again.`
      : "Publish them so everyone gets the same mods.",
  )}
{:else if s?.state === "frozen"}
  {@render bar(
    "plain",
    "shield",
    "This instance is frozen",
    "Its owner left and nobody could take over. Your copy keeps working, but there won't be updates.",
  )}
{/if}
