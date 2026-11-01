<script lang="ts">
  import { api } from "$lib/api";
  import { back, forgetInstance, go, nav } from "$lib/nav.svelte";
  import {
    chooseAction,
    confirmAction,
    type Choice,
  } from "$lib/confirm.svelte";
  import { openDialog } from "$lib/dialogs.svelte";
  import { openLabel, plural, versionLabel } from "$lib/format";
  import { canPlay, play } from "$lib/play";
  import { resource } from "$lib/resource.svelte";
  import { instances, social } from "$lib/stores";
  import type { Id } from "$lib/types";
  import ContentIcon from "../components/ContentIcon.svelte";
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

  // The files changed (a scan finished, the watcher saw a change, names arrived from
  // Modrinth, an update or restore ended): reload the detail. The store refreshes the
  // summary on the same event, but the content list lives only on this page.
  // An $effect that returns a function: Svelte calls it when the page goes away, which
  // is exactly when we should stop listening (api.subscribe returns the unsubscribe).
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

  const stopTracking = { confirmLabel: "Remove", danger: true };

  async function untrack() {
    if (!summary) return;
    const forget = `Atlas forgets it and deletes its backups. It stays in ${summary.launcher.name}, untouched.`;
    let choice: Choice | null;
    if (!sharedId) {
      choice = await chooseAction({
        ...stopTracking,
        title: `Remove ${summary.name} from Atlas?`,
        message: `${forget} You can add it again later.`,
      });
    } else if (summary.sync?.role === "owner") {
      // Stopping tracking here never ends the sharing: you may have it on another PC.
      const shared = social.shared.find((s) => s.id === sharedId);
      const people =
        shared?.members.filter(
          (m) => m.role !== "owner" && m.status === "active",
        ).length ?? 0;
      choice = await chooseAction({
        ...stopTracking,
        title: `Remove ${summary.name} from Atlas?`,
        message: `${forget} It stays shared${people ? ` with ${plural(people, "person", "people")}` : ""}, and you can add it back from Shared to send updates again.`,
        alternativeLabel: "Stop sharing too",
      });
      if (
        choice === "alternative" &&
        !(await confirmAction({
          title: `Stop sharing ${summary.name}?`,
          message:
            "Nobody gets updates anymore. Everyone keeps their copy as a normal instance.",
          confirmLabel: "Stop sharing",
          danger: true,
        }))
      )
        return;
    } else {
      choice = await chooseAction({
        ...stopTracking,
        title: `Remove ${summary.name} from Atlas?`,
        message: `${forget} You still get its updates, so you can add it back from Shared.`,
        alternativeLabel: "Stop updates too",
      });
    }
    if (!choice) return;
    try {
      if (choice === "alternative" && sharedId)
        await (summary.sync?.role === "owner"
          ? api.archiveSharedInstance(sharedId)
          : api.leaveSharedInstance(sharedId));
      await api.untrackInstance(id);
    } catch (e) {
      error = (e as Error).message;
      await instances.load();
      return;
    }
    await social.load();
    await instances.load();
    // This page no longer exists: remove it from history and leave without remembering it.
    forgetInstance(id);
    go({ page: "home" }, { replace: true });
  }

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
            {
              label: "Export as .mrpack…",
              icon: "download",
              onselect: () => run(() => api.exportMrpack(id)),
            },
          ]),
      "divider",
      ...(s && s.role !== "owner"
        ? [
            {
              label: "Stop getting updates",
              hint: `Leave ${s.owner?.displayName ?? "the owner"}'s pack. Your files stay as they are.`,
              danger: true,
              onselect: async () => {
                const ok = await confirmAction({
                  title: `Stop getting updates for ${summary.name}?`,
                  message: `You leave ${s.owner?.displayName ?? "the owner"}'s pack. Your copy stays here and keeps working, but won't change when they send updates. They'd have to invite you again.`,
                  confirmLabel: "Stop updates",
                  danger: true,
                });
                if (ok) run(() => api.leaveSharedInstance(s.sharedInstanceId));
              },
            },
          ]
        : []),
      ...(s && s.role === "owner"
        ? [
            {
              label: "Stop sharing",
              hint: "Friends keep their copy but get no more updates.",
              danger: true,
              onselect: async () => {
                const ok = await confirmAction({
                  title: `Stop sharing ${summary.name}?`,
                  message:
                    "Nobody gets updates anymore, you included. Everyone keeps their copy as a normal instance.",
                  confirmLabel: "Stop sharing",
                  danger: true,
                });
                if (ok)
                  run(() => api.archiveSharedInstance(s.sharedInstanceId));
              },
            },
          ]
        : []),
      {
        label: "Remove from Atlas",
        hint: `Atlas forgets it. It stays in ${summary.launcher.name}.`,
        danger: true,
        onselect: untrack,
      },
      ...(s && s.role !== "owner"
        ? [
            "divider" as const,
            {
              label: "Report…",
              onselect: () =>
                openDialog({
                  kind: "report",
                  about: {
                    kind: "shared_instance",
                    id: s.sharedInstanceId,
                    name: summary.name,
                  },
                }),
            },
          ]
        : []),
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
  {#if backLabel}
    <button
      onclick={back}
      class="mb-4.5 inline-flex items-center gap-1.5 text-[13px] text-(--text-muted) hover:text-(--text-primary)"
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
  <div class="flex items-start gap-5">
    <ContentIcon
      src={summary.icon}
      seed={summary.id}
      class="size-22 rounded-xl {summary.status === 'missing'
        ? 'opacity-40 grayscale'
        : ''}"
    />

    <div
      class="flex min-w-0 flex-1 flex-wrap items-start justify-between gap-x-5 gap-y-3"
    >
      <div class="min-w-60 flex-1 pt-1">
        <h1
          class="mb-2 truncate font-(family-name:--font-display) text-2xl font-semibold tracking-tight"
        >
          {summary.name}
        </h1>
        <div
          class="flex flex-wrap gap-x-4 gap-y-1.5 text-[13px] text-(--text-muted)"
        >
          <span>{versionLabel(summary.loader, summary.minecraftVersion)}</span>
          <span>{summary.launcher.name}</span>
          {#if summary.sync}
            <span>
              {summary.sync.role === "owner"
                ? "Shared by you"
                : `From ${summary.sync.owner?.displayName ?? "a deleted user"}`}
            </span>
          {:else}
            <span>Not shared</span>
          {/if}
          {#if (summary.totalPlaytimeSeconds ?? 0) >= 1800}
            <span
              >{plural(
                Math.round((summary.totalPlaytimeSeconds ?? 0) / 3600),
                "hour",
              )} played</span
            >
          {/if}
        </div>
      </div>

      <div class="flex items-center gap-2 pt-1.5">
        <!-- Only Prism and MultiMC start the game; elsewhere Play opens the launcher, so a
             separate Open button would do the same thing. -->
        {#if summary.status !== "missing" && (summary.launcher.kind === "prism" || summary.launcher.kind === "multimc")}
          <Button
            variant="ghost"
            disabled={busy}
            onclick={() => run(() => api.openInLauncher(id))}
          >
            <Icon name="external" class="size-3.5" />{openLabel(
              summary.launcher,
            )}
          </Button>
        {/if}
        {#if summary.launcher.kind !== "folder" && summary.status !== "missing"}
          <Button
            disabled={busy || !canPlay(summary)}
            title={summary.isRunning
              ? "Minecraft is running"
              : `Play in ${summary.launcher.name}`}
            onclick={() => play(summary)}
          >
            <Icon
              name="play"
              class="size-3.5 text-(--online)"
            />{summary.isRunning ? "Playing" : "Play"}
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
            >Send update</Button
          >
        {:else if summary.sync.role !== "viewer" && summary.sync.state !== "frozen"}
          <Button onclick={() => openDialog({ kind: "invite", instanceId: id })}
            ><Icon name="users" class="size-3.5" />Invite</Button
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
  <div class="mt-6.5">
    <Tabs
      bind:value={tab}
      tabs={[
        { id: "content", label: "Content" },
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
      <!-- Settings and a short list: a readable width, not the whole window. -->
      <div class="max-w-3xl">
        <SharingTab instanceId={summary.id} sync={summary.sync} />
      </div>
    {:else}
      <div class="py-12 text-center text-(--text-muted)">
        <b class="block text-sm text-(--text-primary)"
          >Only you have this instance</b
        >
        Share it and friends get the same mods, plus every update you publish.
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
