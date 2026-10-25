<script lang="ts">
  import { api } from "$lib/api";
  import { fileSize, plural, timeAgo } from "$lib/format";
  import { go } from "$lib/nav.svelte";
  import { resource } from "$lib/resource.svelte";
  import { instances } from "$lib/stores";
  import type { Id, SyncProgress } from "$lib/types";
  import BlockArt from "../components/BlockArt.svelte";
  import Button from "../components/Button.svelte";
  import Head from "../components/Head.svelte";
  import Icon from "../components/Icon.svelte";
  import OptionalPicker from "../components/OptionalPicker.svelte";

  let { id }: { id: Id } = $props();

  const summary = $derived(instances.byId(id));
  const sharedId = $derived(summary?.sync?.sharedInstanceId ?? null);

  // Three requests, one loading state. Promise.all starts them together and
  // resolves when all are done; destructuring gives each result a name.
  const review = resource(async () => {
    if (!sharedId) return null;
    const [preview, shared, detail] = await Promise.all([
      api.previewSync(id),
      api.getSharedInstance(sharedId),
      api.getInstance(id),
    ]);
    return {
      preview,
      latest: shared.revisions[0], // newest first: who published it and their note
      extras: detail.content.filter((c) => !c.isManaged), // the user's own files, kept on update
    };
  });

  // Per-file choice for config conflicts, keyed by path. Missing = take theirs.
  // A $state object tracks each key separately, so one radio only re-renders its own row.
  let choices = $state<Record<string, "mine" | "theirs">>({});
  // New optional items this update offers. Nothing ticked unless the user chooses.
  let installOptional = $state<string[]>([]);

  let started = $state(false);
  let error = $state<string | null>(null);
  let cancelled = $state(false);

  const hasUpdate = $derived(
    !!summary?.sync &&
      summary.sync.syncedRevision < summary.sync.latestRevision,
  );
  const syncing = $derived(summary?.status === "syncing");
  const progress = $derived(summary?.progress ?? null);

  // When the update we started finishes, return to the instance page.
  // $effect is the right tool: navigation is a side effect reacting to a state change.
  //
  // Careful: right after clicking, the store still says "ready" — it only switches to
  // "syncing" when the first progress event arrives. So "finished" means we've seen
  // "syncing" and then "ready" again, not just "ready".
  let sawSyncing = false; // plain variable: it's bookkeeping, nothing renders it
  $effect(() => {
    const status = summary?.status;
    if (!started) return;
    if (status === "syncing") sawSyncing = true;
    else if (status === "ready" && sawSyncing) {
      if (cancelled) {
        started = false;
        sawSyncing = false;
        cancelled = false;
        error = "Update cancelled. Your files weren't changed.";
        return;
      }
      // A failure arrives just before "ready": stay here and say why.
      const failed = instances.failures[id];
      if (failed) {
        started = false;
        sawSyncing = false;
        error = `The update didn't finish: ${failed} Your files weren't changed.`;
        return;
      }
      // replace: Back from the instance shouldn't return to a finished review.
      go({ page: "instance", id }, { replace: true });
    }
  });

  function conflictOptions(
    c: { mine: string; theirs: string },
    publisher: string,
  ) {
    return [
      { id: "mine", label: "Keep mine", value: c.mine },
      { id: "theirs", label: `Use ${publisher}'s`, value: c.theirs },
    ] as const;
  }

  const STAGES: Record<SyncProgress["stage"], string> = {
    waiting_for_game_exit: "Waiting for you to close Minecraft…",
    waiting_for_launcher_exit: "Waiting for the launcher to close…",
    backup: "Backing up files…",
    download: "Downloading…",
    waiting_for_manual_downloads: "Waiting for manual downloads…",
    apply: "Applying changes…",
  };

  async function apply() {
    error = null;
    started = true;
    try {
      await api.syncInstance(id, { configChoices: choices, installOptional });
      // From here, progress arrives through live events into the store (see connectStores).
    } catch (e) {
      started = false;
      error = (e as Error).message;
    }
  }
</script>

<div class="max-w-205">
  <button
    onclick={() => go({ page: "instance", id })}
    class="mb-4.5 inline-flex items-center gap-1.5 text-[12.5px] text-(--text-muted) hover:text-(--text-primary)"
  >
    <Icon name="chev" class="size-3.5 rotate-180" />{summary?.name ?? "Back"}
  </button>

  {#if !summary}
    <p class="text-(--text-muted)">This instance isn't tracked anymore.</p>
  {:else if !hasUpdate && !started}
    <div class="py-12 text-center text-(--text-muted)">
      <b class="block text-sm text-(--text-primary)"
        >You're on the latest version</b
      >
      {summary.name} is on v{summary.sync?.syncedRevision}. Nothing to update.
      <div>
        <Button class="mt-4" onclick={() => go({ page: "instance", id })}
          >Back to {summary.name}</Button
        >
      </div>
    </div>
  {:else if review.loading && !review.data}
    <p class="text-(--text-muted)">Working out what changed…</p>
  {:else if review.error}
    <p class="text-(--danger)">
      Couldn't load the update: {review.error}
      <Button size="sm" class="ml-2" onclick={review.reload}>Try again</Button>
    </p>
  {:else if review.data}
    {@const { preview, latest, extras } = review.data}
    {@const changes = preview.changes}
    {@const total =
      changes.added.length + changes.updated.length + changes.removed.length}

    <!-- ── Header ── -->
    <div class="mb-5.5 flex items-center gap-4">
      <BlockArt seed={id} class="size-14 rounded-lg" />
      <div>
        <h1
          class="font-(family-name:--font-display) text-[22px] font-semibold tracking-tight"
        >
          Update to v{preview.toRevision}
        </h1>
        <p class="mt-1 text-(--text-muted)">
          {summary.name} · you're on v{preview.fromRevision} · {summary.launcher
            .name}
        </p>
      </div>
    </div>

    <!-- ── Publisher's note ── -->
    {#if latest}
      <div class="mb-5.5 flex gap-3 rounded-lg bg-(--bg-raise) px-4 py-3.5">
        {#if latest.createdBy}<Head
            user={latest.createdBy}
            size={28}
            ring="var(--bg-raise)"
          />{/if}
        <div>
          <p>{latest.changelog ?? "No description."}</p>
          <small class="text-(--text-faint)"
            >{latest.createdBy?.displayName ?? "Deleted user"} · {timeAgo(
              latest.createdAt,
            )}</small
          >
        </div>
      </div>
    {/if}

    <!-- ── Things that need attention ── -->
    {#if preview.gameRunning || summary.isRunning}
      <div class="mb-5.5 rounded-lg bg-(--warn)/10 px-4 py-3 text-[12.5px]">
        <b class="font-semibold text-(--warn)"
          >Minecraft is running this instance.</b
        >
        Close the game first. If you start the update now, Atlas waits until it's
        closed.
      </div>
    {/if}

    {#if preview.launcherChange}
      {@const lc = preview.launcherChange}
      <div class="mb-5.5 rounded-lg bg-(--bg-raise) px-4 py-3 text-[12.5px]">
        <b class="font-semibold"
          >This version needs a different setup in {summary.launcher.name}</b
        >
        <ul class="mt-1 list-disc pl-5 text-(--text-muted)">
          {#if lc.minecraftVersion}<li>
              Minecraft {lc.minecraftVersion.from} → {lc.minecraftVersion.to}
            </li>{/if}
          {#if lc.loaderVersion}<li>
              Loader {lc.loaderVersion.from} → {lc.loaderVersion.to}
            </li>{/if}
        </ul>
        <p class="mt-1 text-(--text-muted)">
          {lc.automatic
            ? "Atlas changes this in the launcher for you."
            : `Change it in ${summary.launcher.name}'s instance settings after updating.`}
        </p>
      </div>
    {/if}

    {#if preview.overwrittenLocalChanges.length}
      <div class="mb-5.5 rounded-lg bg-(--warn)/10 px-4 py-3 text-[12.5px]">
        <b class="font-semibold text-(--warn)"
          >This update undoes your changes to:</b
        >
        {preview.overwrittenLocalChanges.join(", ")}. They're backed up first.
      </div>
    {/if}

    {#each preview.configConflicts as conflict (conflict.path)}
      {@const choice = choices[conflict.path] ?? "theirs"}
      <div class="mb-5.5 rounded-lg bg-(--warn)/10 px-4 py-3.5">
        <b class="font-semibold"
          >You changed a config file this update also changes</b
        >
        <div
          class="mt-0.5 font-(family-name:--font-mono) text-xs text-(--text-muted)"
        >
          {conflict.path}
        </div>
        <div class="mt-3 grid grid-cols-2 gap-2.5">
          {#each conflictOptions(conflict, latest?.createdBy?.displayName ?? "the owner") as option (option.id)}
            <label
              class="cursor-pointer rounded-md border bg-(--bg) px-3 py-2.5 {choice ===
              option.id
                ? 'border-(--warn)'
                : 'border-(--line-2)'}"
            >
              <input
                type="radio"
                name={conflict.path}
                checked={choice === option.id}
                onchange={() => (choices[conflict.path] = option.id)}
                class="mr-2 accent-(--warn)"
              />
              {option.label}
              <code
                class="mt-1.5 block bg-transparent p-0 font-(family-name:--font-mono) text-[11.5px] text-(--text-muted)"
                >{option.value}</code
              >
            </label>
          {/each}
        </div>
      </div>
    {/each}

    <!-- ── The diff ── -->
    {#snippet group(title: string, count: number)}
      <h2
        class="mb-1 flex gap-2 text-[12.5px] font-semibold text-(--text-muted)"
      >
        {title} <span class="font-normal text-(--text-faint)">{count}</span>
      </h2>
    {/snippet}
    {#snippet row(symbol: string, cls: string, name: string, version: string)}
      <div
        class="grid grid-cols-[18px_minmax(0,1fr)_auto] items-center gap-3 rounded-md px-2 py-2 hover:bg-(--bg-hover)"
      >
        <span
          class="text-center font-(family-name:--font-mono) font-semibold {cls}"
          >{symbol}</span
        >
        <span>{name}</span>
        <span class="font-(family-name:--font-mono) text-xs text-(--text-muted)"
          >{version}</span
        >
      </div>
    {/snippet}

    {#if changes.added.length}
      <section class="mb-5">
        {@render group("Added", changes.added.length)}
        <!-- Unkeyed: names aren't unique (two files can share one). -->
        {#each changes.added as name}{@render row(
            "+",
            "text-(--online)",
            name,
            "",
          )}{/each}
      </section>
    {/if}
    {#if changes.updated.length}
      <section class="mb-5">
        {@render group("Updated", changes.updated.length)}
        {#each changes.updated as u}{@render row(
            "↑",
            "text-(--accent)",
            u.name,
            `${u.from} → ${u.to}`,
          )}{/each}
      </section>
    {/if}
    {#if changes.removed.length}
      <section class="mb-5">
        {@render group("Removed", changes.removed.length)}
        {#each changes.removed as name}{@render row(
            "−",
            "text-(--danger)",
            name,
            "",
          )}{/each}
      </section>
    {/if}
    {#if extras.length}
      <section class="mb-5">
        {@render group("Your additions stay", extras.length)}
        {#each extras as c (c.id)}{@render row(
            "·",
            "text-(--text-faint)",
            c.name,
            c.versionLabel ?? "",
          )}{/each}
      </section>
    {/if}

    <OptionalPicker
      items={preview.newOptional}
      bind:selected={installOptional}
      title="New optional extras"
    />

    <!-- ── Sticky action bar: stays at the bottom of the scroll area ── -->
    <div
      class="sticky bottom-0 -mx-9 mt-4 flex items-center justify-between gap-3.5 bg-linear-to-t from-(--bg) from-70% to-transparent px-9 py-3.5"
    >
      {#if syncing && progress}
        <span class="text-[12.5px] text-(--text-muted)"
          >{STAGES[progress.stage]}</span
        >
        <div class="flex items-center gap-3">
          <div class="h-1 w-56 overflow-hidden rounded-full bg-(--line-2)">
            <div
              class="h-full bg-(--accent) transition-[width] duration-300"
              style="width: {progress.percent}%"
            ></div>
          </div>
          {#if progress.stage !== "apply"}
            <Button
              size="sm"
              variant="ghost"
              disabled={cancelled}
              onclick={() => {
                cancelled = true;
                api.cancelJob(id).catch(() => (cancelled = false));
              }}>Cancel</Button
            >
          {/if}
        </div>
      {:else}
        <span
          class="text-[12.5px] {error
            ? 'text-(--danger)'
            : 'text-(--text-muted)'}"
        >
          {error ??
            `${plural(total, "change")} · ${fileSize(preview.downloadBytes)} download · changed files are backed up first`}
        </span>
        <div class="flex gap-2">
          <Button variant="ghost" onclick={() => go({ page: "instance", id })}
            >Not now</Button
          >
          <Button variant="warn" size="lg" disabled={started} onclick={apply}
            >Update to v{preview.toRevision}</Button
          >
        </div>
      {/if}
    </div>
  {/if}
</div>
