<script lang="ts">
  import { iconSeed } from "$lib/pixel";
  import { api } from "$lib/api";
  import { back, forgetInstance, go, nav } from "$lib/nav.svelte";
  import { confirmAction } from "$lib/confirm.svelte";
  import { openDialog } from "$lib/dialogs.svelte";
  import { openLabel, plural, timeAgo, versionLabel } from "$lib/format";
  import { instanceItems, removeFromAtlas } from "$lib/instanceMenu";
  import { canPlay, play, unfinished } from "$lib/play";
  import { resource } from "$lib/resource.svelte";
  import { instances, social } from "$lib/stores";
  import type { Id } from "$lib/types";
  import ContentIcon from "../components/ContentIcon.svelte";
  import Button from "../components/Button.svelte";
  import Icon from "../components/Icon.svelte";
  import Menu from "../components/Menu.svelte";
  import Tabs from "../components/Tabs.svelte";
  import ContentTab from "../components/instance/ContentTab.svelte";
  import FilesTab from "../components/instance/FilesTab.svelte";
  import HistoryTab from "../components/instance/HistoryTab.svelte";
  import SharingTab from "../components/instance/SharingTab.svelte";
  import SyncBar from "../components/instance/SyncBar.svelte";

  let { id }: { id: Id } = $props();

  // Two sources, on purpose:
  // - `summary` comes from the store, which live events keep current (progress, running…).
  // - `detail` is the heavy data (content list, backups), loaded for this page only.
  const summary = $derived(instances.byId(id));
  const detail = resource(() => api.getInstance(id));

  let tab = $state<"content" | "files" | "sharing" | "history">("content");
  // Owners and editors choose which config and other files friends get.
  const canSetFiles = $derived(
    !!summary?.sync && summary.sync.role !== "viewer",
  );
  let busy = $state(false);
  let error = $state<string | null>(null);

  // A new instance was opened: go back to the first tab.
  $effect(() => {
    id;
    tab = "content";
    error = null;
  });

  // The files changed (a scan finished, the watcher saw a change, names arrived from
  // Modrinth, an update or restore ended): reload the detail. The store refreshes the
  // summary on the same event, but the content list lives only on this page.
  $effect(() =>
    api.subscribe((e) => {
      if (e.type === "instance.changed" && e.instanceId === id) detail.reload();
    }),
  );

  // Where the back link goes, named so it's clear ("← Home", "← Shared"…).
  const backLabel = $derived.by(() => {
    const p = nav.previous;
    if (!p) return null;
    if (p.page === "home") return "Home";
    if (p.page === "shared") return "Shared";
    if (p.page === "friends") return "Friends";
    if (p.page === "review") return "Review update";
    return instances.byId(p.id)?.name ?? "Back";
  });

  // Why changing files is blocked right now, if it is.
  const lockedReason = $derived(
    !summary
      ? null
      : summary.status === "missing"
        ? "The instance's folder is missing."
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

  /** The shared instance this is linked to, if any (dev-folder links aren't on the server). */
  const sharedId = $derived(
    summary?.sync && !summary.sync.sharedInstanceId.startsWith("dev:")
      ? summary.sync.sharedInstanceId
      : null,
  );

  const untrack = () => summary && removeFromAtlas(summary);

  /** A shared pack whose folder is gone: forget this copy and add a fresh one. */
  async function setUpAgain() {
    if (!summary?.sync) return;
    const shared = summary.sync.sharedInstanceId;
    try {
      await api.untrackInstance(id);
    } catch (e) {
      error = (e as Error).message;
      return;
    }
    await Promise.all([social.load(), instances.load()]);
    forgetInstance(id);
    go({ page: "home" }, { replace: true });
    openDialog({ kind: "accept", sharedInstanceId: shared });
  }

  const menuItems = $derived(
    summary ? instanceItems(summary, { onPage: true, run }) : [],
  );
</script>

{#if !summary}
  {#if instances.loaded}
    <div class="py-16 text-center text-(--text-muted)">
      <b class="block text-sm text-(--text-primary)"
        >This instance isn't in Atlas anymore.</b
      >
      <Button class="mt-4" onclick={() => go({ page: "home" })}
        >Back to Home</Button
      >
    </div>
  {/if}
{:else}
  {#if backLabel}
    <button
      onclick={back}
      class="-mt-0.75 mb-3.75 inline-flex items-center gap-1.5 py-0.75 text-[13px] text-(--text-muted) hover:text-(--text-primary)"
    >
      <Icon name="chev" class="size-3.5 rotate-180" />{backLabel}
    </button>
  {/if}

  {#if instances.failures[id]}
    <div
      role="alert"
      class="mb-5 flex items-start gap-3 rounded-lg border border-(--danger)/30 bg-(--danger)/8 px-4 py-3 text-[13px]"
    >
      <span class="mt-0.5 text-(--danger)"
        ><Icon name="warn" class="size-4" /></span
      >
      <span class="min-w-0 flex-1">
        <b class="font-semibold">The last update didn't finish.</b>
        {instances.failures[id]} Your files weren't changed.
      </span>
      <button
        onclick={() => instances.dismissFailure(id)}
        aria-label="Dismiss"
        class="grid size-6 place-items-center rounded-md text-(--text-faint) hover:bg-(--bg-hover) hover:text-(--text-primary)"
      >
        <Icon name="close" class="size-3.5" />
      </button>
    </div>
  {/if}

  <!-- ── Header ─────────────────────────────────────────────────────── -->
  <div class="flex items-center gap-4">
    <ContentIcon
      src={summary.icon}
      seed={iconSeed(summary)}
      class="size-16 shrink-0 rounded-xl {summary.status === 'missing'
        ? 'opacity-40 grayscale'
        : ''}"
    />

    <div class="flex min-w-0 flex-1 flex-wrap items-center gap-x-5 gap-y-3">
      <div class="min-w-60 flex-1">
        <h1
          class="truncate font-(family-name:--font-display) text-2xl leading-tight font-semibold tracking-tight"
          title={summary.name}
        >
          {summary.name}
        </h1>
        <!-- What it is, where it lives, who it's shared with, then how much it's played. -->
        <div
          class="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-[13px] text-(--text-muted)"
        >
          <span>{versionLabel(summary.loader, summary.minecraftVersion)}</span>
          <span class="text-(--text-faint)" aria-hidden="true">·</span>
          <span>{summary.launcher.name}</span>
          <span
            class="ml-1.5 inline-flex h-5.5 items-center gap-1.5 rounded-full px-2.25 font-medium
              {summary.sync
              ? 'bg-(--accent)/12 text-(--accent)'
              : 'bg-(--bg-raise-2) text-(--text-muted)'}"
          >
            <Icon name={summary.sync ? "share" : "cube"} class="size-3" />
            {#if summary.sync}
              {summary.sync.role === "owner"
                ? "Shared by you"
                : `From ${summary.sync.owner?.displayName ?? "a deleted user"}`}
            {:else if summary.sharedElsewhere}
              Shared on another account
            {:else}
              Not shared
            {/if}
          </span>
          {#if summary.lastPlayedAt || (summary.totalPlaytimeSeconds ?? 0) >= 1800}
            <span
              class="ml-1.5 inline-flex items-center gap-1.5 text-(--text-faint)"
            >
              <Icon name="clock" class="size-3" />
              {#if (summary.totalPlaytimeSeconds ?? 0) >= 1800}
                {plural(
                  Math.round((summary.totalPlaytimeSeconds ?? 0) / 3600),
                  "hour",
                )} played{summary.lastPlayedAt ? "," : ""}
              {/if}
              {#if summary.lastPlayedAt}
                {(summary.totalPlaytimeSeconds ?? 0) >= 1800
                  ? "last"
                  : "Played"}
                {timeAgo(summary.lastPlayedAt)}
              {/if}
            </span>
          {/if}
        </div>
      </div>

      <div class="flex items-center gap-2">
        <!-- Only Prism and MultiMC start the game; elsewhere Play opens the launcher, so a
             separate Open button would do the same thing. -->
        {#if summary.status !== "missing" && (summary.launcher.kind === "prism" || summary.launcher.kind === "multimc")}
          <Button
            variant="ghost"
            disabled={busy}
            icon="external"
            onclick={() => run(() => api.openInLauncher(id))}
          >
            {openLabel(summary.launcher)}
          </Button>
        {/if}
        {#if summary.launcher.kind !== "folder" && summary.status !== "missing"}
          <Button
            disabled={busy || !canPlay(summary)}
            title={summary.isRunning
              ? "Minecraft is running"
              : unfinished(summary)
                ? "Finish setting it up first"
                : `Play in ${summary.launcher.name}`}
            onclick={() => play(summary)}
          >
            {#snippet icon()}
              <Icon name="play" class="size-3.5 text-(--online)" />
            {/snippet}
            {summary.isRunning ? "Playing" : "Play"}
          </Button>
        {/if}

        <!-- The main action depends on the sync state. -->
        {#if !summary.sync}
          {#if !summary.sharedElsewhere}
            <Button
              variant="primary"
              disabled={summary.status !== "ready"}
              icon="share"
              onclick={() => openDialog({ kind: "share", instanceId: id })}
            >
              Share
            </Button>
          {/if}
        {:else if summary.sync.state === "local_changes" && summary.sync.role !== "viewer"}
          <Button
            variant="primary"
            onclick={() => openDialog({ kind: "publish", instanceId: id })}
            >Send update</Button
          >
        {:else if summary.sync.role !== "viewer" && summary.sync.state !== "frozen"}
          <Button
            icon="users"
            onclick={() => openDialog({ kind: "invite", instanceId: id })}
            >Invite</Button
          >
        {/if}

        <Menu items={menuItems} />
      </div>
    </div>
  </div>

  <SyncBar
    instance={summary}
    {busy}
    onupdate={() => go({ page: "review", id })}
    onrevert={async () => {
      const ok = await confirmAction({
        title: "Undo your changes?",
        message:
          "Files you added, removed or changed go back to the shared version. Atlas backs up the current files first, so you can restore them from History.",
        confirmLabel: "Undo changes",
      });
      if (ok) run(() => api.revertLocalChanges(id));
    }}
    onuntrack={untrack}
    onsetup={sharedId ? setUpAgain : undefined}
  />

  {#if error}
    <p class="mt-3 text-[13px] text-(--danger)" role="alert">{error}</p>
  {/if}

  <!-- ── Tabs ───────────────────────────────────────────────────────── -->
  <div class="mt-5">
    <Tabs
      bind:value={tab}
      tabs={[
        { id: "content", label: "Content" },
        ...(canSetFiles ? [{ id: "files" as const, label: "Files" }] : []),
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
  {:else if tab === "content" || (tab === "files" && !canSetFiles)}
    <ContentTab
      instance={detail.data}
      {lockedReason}
      onchanged={() => instances.load()}
    />
  {:else if tab === "files" && canSetFiles}
    <div class="max-w-4xl">
      <!-- Read again after each update, so what was sent loses its dot. -->
      {#key summary.sync?.latestRevision}
        <FilesTab instanceId={summary.id} />
      {/key}
    </div>
  {:else if tab === "sharing"}
    {#if summary.sync}
      <!-- Settings and a short list: a readable width, not the whole window. -->
      <div class="max-w-3xl">
        <SharingTab instanceId={summary.id} sync={summary.sync} />
      </div>
    {:else if summary.sharedElsewhere}
      <div class="py-12 text-center text-(--text-muted)">
        <b class="block text-sm text-(--text-primary)"
          >Shared on another account</b
        >
        You shared or joined this while signed in to another account. Sign in to that
        one to send or get its updates. Your files here are fine.
      </div>
    {:else}
      <div class="py-12 text-center text-(--text-muted)">
        <b class="block text-sm text-(--text-primary)"
          >Only you have this instance</b
        >
        Share it and friends get the same mods, plus every update you send.
      </div>
    {/if}
  {:else}
    <div class="max-w-3xl">
      <HistoryTab
        instance={detail.data}
        {lockedReason}
        onrestored={() => instances.load()}
      />
    </div>
  {/if}
{/if}
