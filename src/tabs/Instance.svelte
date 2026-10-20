<script lang="ts">
  import { api } from "$lib/api";
  import { forgetInstance, go } from "$lib/nav.svelte";
  import { openDialog } from "$lib/dialogs.svelte";
  import { plural, versionLabel } from "$lib/format";
  import { resource } from "$lib/resource.svelte";
  import { instances } from "$lib/stores";
  import type { Id } from "$lib/types";
  import BlockArt from "../components/BlockArt.svelte";
  import Button from "../components/Button.svelte";
  import Icon from "../components/Icon.svelte";
  import Menu, { type MenuItem } from "../components/Menu.svelte";
  import Tabs from "../components/Tabs.svelte";
  import ContentTab from "../components/instance/ContentTab.svelte";
  import HistoryTab from "../components/instance/HistoryTab.svelte";
  import SharingTab from "../components/instance/SharingTab.svelte";
  import SyncBar from "../components/instance/SyncBar.svelte";

  let { id }: { id: Id } = $props();

  // Two sources, on purpose:
  // - `summary` comes from the store, which live events keep current (progress, running…).
  // - `detail` is the heavy data (content list, backups), loaded for this page only.
  const summary = $derived(instances.byId(id));
  const detail = resource(() => api.getInstance(id));

  let tab = $state<"content" | "sharing" | "history">("content");
  let busy = $state(false);
  let error = $state<string | null>(null);

  // A new instance was opened: go back to the first tab.
  $effect(() => {
    id; // reading `id` makes it a dependency
    tab = "content";
    error = null;
  });

  // When an update or restore finishes (syncing → ready), the files changed: reload the detail.
  let lastStatus: string | undefined;
  $effect(() => {
    const status = summary?.status;
    if (lastStatus === "syncing" && status === "ready") detail.reload();
    lastStatus = status;
  });

  // Why changing files is blocked right now, if it is.
  const lockedReason = $derived(
    !summary
      ? null
      : summary.status === "missing"
        ? "The folder is missing."
        : summary.status === "syncing"
          ? "Wait for the update to finish."
          : summary.isRunning
            ? "Close Minecraft to change mods."
            : null,
  );

  /** Runs an action with busy/error handling, then refreshes the store and this page. */
  async function run(action: () => Promise<unknown>) {
    busy = true;
    error = null;
    try {
      await action();
      await instances.load();
      detail.reload();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }

  async function untrack() {
    if (!summary) return;
    if (
      !confirm(
        `Stop tracking ${summary.name}? Its folder in ${summary.launcher.name} isn't touched.`,
      )
    )
      return;
    await api.untrackInstance(id);
    await instances.load();
    // This page no longer exists: remove it from history and leave without remembering it.
    forgetInstance(id);
    go({ page: "home" }, { replace: true });
  }

  const menuItems = $derived.by((): MenuItem[] => {
    if (!summary) return [];
    const s = summary.sync;
    const missing = summary.status === "missing";
    return [
      ...(missing
        ? []
        : [
            {
              label: "Show in File Explorer",
              icon: "folders",
              onselect: () => run(() => api.openInstanceFolder(id)),
            },
            {
              label: "Rescan folder",
              icon: "refresh",
              onselect: () => run(() => api.rescanInstance(id)),
            },
          ]),
      "divider",
      ...(s && s.role !== "owner"
        ? [
            {
              label: "Leave shared instance",
              danger: true,
              onselect: () =>
                run(() => api.leaveSharedInstance(s.sharedInstanceId)),
            },
          ]
        : []),
      ...(s && s.role === "owner"
        ? [
            {
              label: "Stop sharing",
              danger: true,
              onselect: () =>
                run(() => api.archiveSharedInstance(s.sharedInstanceId)),
            },
          ]
        : []),
      { label: "Stop tracking", danger: true, onselect: untrack },
    ];
  });
</script>

{#if !summary}
  {#if instances.loaded}
    <div class="py-16 text-center text-(--text-muted)">
      <b class="block text-sm text-(--text-primary)"
        >This instance isn't tracked anymore.</b
      >
      <Button class="mt-4" onclick={() => go({ page: "home" })}
        >Back to Home</Button
      >
    </div>
  {/if}
{:else}
  <!-- ── Header ─────────────────────────────────────────────────────── -->
  <div class="flex items-start gap-5">
    <BlockArt
      seed={summary.id}
      class="size-22 rounded-xl {summary.status === 'missing'
        ? 'opacity-40 grayscale'
        : ''}"
    />

    <div class="min-w-0 flex-1 pt-1">
      <h1
        class="mb-2 truncate font-(family-name:--font-display) text-2xl font-semibold tracking-tight"
      >
        {summary.name}
      </h1>
      <div
        class="flex flex-wrap gap-x-4 gap-y-1.5 text-[12.5px] text-(--text-muted)"
      >
        <span>{versionLabel(summary.loader, summary.minecraftVersion)}</span>
        <span>{summary.launcher.name}</span>
        {#if summary.sync}
          <span>
            {summary.sync.role === "owner"
              ? "Shared by you"
              : `From ${summary.sync.owner?.displayName ?? "a deleted user"}`}
            · v{summary.sync.syncedRevision}
          </span>
        {:else}
          <span>Only on this PC</span>
        {/if}
        {#if summary.totalPlaytimeSeconds}
          <span
            >{plural(Math.round(summary.totalPlaytimeSeconds / 3600), "hour")} played</span
          >
        {/if}
      </div>
    </div>

    <div class="flex items-center gap-2 pt-1.5">
      {#if summary.status !== "missing" && summary.launcher.kind !== "folder"}
        <Button
          variant="ghost"
          disabled={busy}
          onclick={() => run(() => api.openInLauncher(id))}
        >
          <Icon name="external" class="size-3.5" />Open in {summary.launcher
            .name}
        </Button>
      {/if}

      <!-- The main action depends on the sync state. -->
      {#if !summary.sync}
        <Button
          variant="primary"
          disabled={summary.status !== "ready"}
          onclick={() => openDialog({ kind: "share", instanceId: id })}
        >
          <Icon name="share" class="size-3.5" />Share
        </Button>
      {:else if summary.sync.state === "local_changes" && summary.sync.role !== "viewer"}
        <Button
          variant="primary"
          onclick={() => openDialog({ kind: "publish", instanceId: id })}
          >Publish v{summary.sync.latestRevision + 1}</Button
        >
      {:else if summary.sync.role !== "viewer" && summary.sync.state !== "frozen"}
        <Button onclick={() => openDialog({ kind: "invite", instanceId: id })}
          ><Icon name="users" class="size-3.5" />Invite</Button
        >
      {/if}

      <Menu items={menuItems} />
    </div>
  </div>

  <SyncBar
    instance={summary}
    {busy}
    onupdate={() => go({ page: "review", id })}
    onrevert={() => run(() => api.revertLocalChanges(id))}
    onuntrack={untrack}
  />

  {#if error}
    <p class="mt-3 text-xs text-(--danger)" role="alert">{error}</p>
  {/if}

  <!-- ── Tabs ───────────────────────────────────────────────────────── -->
  <div class="mt-6.5">
    <Tabs
      bind:value={tab}
      tabs={[
        { id: "content", label: "Content", count: summary.contentCount },
        { id: "sharing", label: "Sharing" },
        { id: "history", label: "History" },
      ]}
    />
  </div>

  {#if detail.error}
    <p class="py-8 text-(--danger)">
      Couldn't load this instance: {detail.error}
      <Button size="sm" class="ml-2" onclick={detail.reload}>Try again</Button>
    </p>
  {:else if !detail.data}
    <p class="py-8 text-(--text-muted)">Loading…</p>
  {:else if tab === "content"}
    <ContentTab
      instance={detail.data}
      {lockedReason}
      onchanged={() => instances.load()}
    />
  {:else if tab === "sharing"}
    {#if summary.sync}
      <SharingTab sync={summary.sync} />
    {:else}
      <div class="py-12 text-center text-(--text-muted)">
        <b class="block text-sm text-(--text-primary)"
          >Only you have this instance</b
        >
        Share it and friends get the same mods, plus every update you publish.
      </div>
    {/if}
  {:else}
    <HistoryTab
      instance={detail.data}
      {lockedReason}
      onrestored={() => instances.load()}
    />
  {/if}
{/if}
