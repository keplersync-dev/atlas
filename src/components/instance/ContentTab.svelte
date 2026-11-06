<script lang="ts">
  import { fuzzyFilter } from "$lib/fuzzy";
  import { KIND, plural } from "$lib/format";
  import { showError } from "$lib/toasts.svelte";
  import { api } from "$lib/api";
  import { openDialog } from "$lib/dialogs.svelte";
  import type {
    ContentKind,
    ContentSource,
    InstanceContent,
    InstanceDetail,
    ShareMode,
  } from "$lib/types";
  import ContentIcon from "../ContentIcon.svelte";
  import Icon from "../Icon.svelte";
  import KindChips from "../KindChips.svelte";
  import Menu from "../Menu.svelte";
  import ShareModeSelect from "../ShareModeSelect.svelte";
  import Switch from "../Switch.svelte";

  let {
    instance,
    lockedReason,
    onchanged,
  }: {
    instance: InstanceDetail;
    /** Why switches are disabled right now (game running, syncing…), or null. */
    lockedReason: string | null;
    /** Called after a change so the parent can refresh sync state. */
    onchanged: () => void;
  } = $props();

  let kind = $state<ContentKind | "all">("all");
  let query = $state("");

  // Where a file comes from. `hint` is the tooltip: what the label means for sharing.
  const SOURCES: Record<
    ContentSource,
    { label: string; dot: string; hint: string }
  > = {
    modrinth: {
      label: "Modrinth",
      dot: "bg-[#1bd96a]",
      hint: "Found on Modrinth. Friends download it from there.",
    },
    curseforge: {
      label: "CurseForge",
      dot: "bg-[#f16436]",
      hint: "Found on CurseForge. Friends download it from there.",
    },
    upload: {
      label: "Uploaded to Atlas",
      dot: "bg-(--accent)",
      hint: "Not on Modrinth or CurseForge, so it's stored on Atlas for friends to download.",
    },
    local: {
      label: "Not identified",
      dot: "border border-(--text-faint)",
      hint: "Atlas couldn't match this file on Modrinth or CurseForge. If you share it, it gets uploaded to Atlas.",
    },
  };

  // Owners and editors decide how each file is shared; viewers see what was decided.
  const canSetModes = $derived(
    !!instance.sync && instance.sync.role !== "viewer",
  );
  // Spare width is shared out (name 2 : version 1 : source 1) so a wide window spaces
  // the columns evenly instead of leaving one wide gap after the name.
  const columns = $derived(
    canSetModes
      ? "grid-cols-[28px_minmax(0,2fr)_minmax(140px,1fr)_224px_36px]"
      : "grid-cols-[28px_minmax(0,2fr)_minmax(140px,1fr)_minmax(110px,1fr)_36px]",
  );
  const ownerName = $derived(
    instance.sync && instance.sync.role === "viewer"
      ? (instance.sync.owner?.displayName ?? "the owner")
      : null,
  );

  /** Owners/editors: has this file's sharing changed since the last publish? */
  const unpublished = (c: InstanceContent) =>
    c.publishedShareMode === null
      ? c.shareMode !== "private"
      : c.shareMode !== c.publishedShareMode;

  /**
   * Differs from the shared version, the same rules as the "local changes" count:
   * a shared file that's disabled, or (owners/editors) a new or re-set file.
   */
  function pending(c: InstanceContent) {
    if (!instance.sync || !c.installed) return false;
    if (c.isManaged && !c.enabled)
      return canSetModes || c.shareMode === "required";
    return canSetModes && c.enabled && unpublished(c);
  }

  // Pending changes go first. Which ones is decided when the list opens or its files
  // change (and once nothing is pending), not on every click, so a row you're editing
  // doesn't jump away.
  let first = $state(new Set<string>());
  let orderedFor = "";
  $effect(() => {
    const files = instance.id + ":" + instance.content.map((c) => c.id).join();
    const now = instance.content.filter(pending);
    if (files !== orderedFor || now.length === 0) {
      orderedFor = files;
      first = new Set(now.map((c) => c.id));
    }
  });

  const visible = $derived.by(() => {
    const ofKind = instance.content.filter(
      (c) => kind === "all" || c.kind === kind,
    );
    // Searching: best match first. Otherwise pending changes first; sort() keeps the
    // original order within each group.
    return query.trim()
      ? fuzzyFilter(ofKind, query, (c) => [c.name])
      : ofKind.sort(
          (a, b) => Number(first.has(b.id)) - Number(first.has(a.id)),
        );
  });

  /** An uploaded file from someone else's shared version: it can be reported. Files from
   *  Modrinth or CurseForge are reported on those sites. */
  const reportable = (c: InstanceContent) =>
    c.isManaged && c.source === "upload" && instance.sync?.role !== "owner";

  /** Viewers: optional items from the shared version can be downloaded or removed freely. */
  const isOptionalExtra = (c: InstanceContent) =>
    !canSetModes && c.isManaged && c.shareMode === "optional";

  /** Optimistic change: update the row now, undo it if the API refuses. */
  async function change(
    apply: () => void,
    undo: () => void,
    call: () => Promise<unknown>,
  ) {
    apply();
    try {
      await call();
      onchanged();
    } catch (e) {
      undo();
      showError((e as Error).message);
    }
  }

  const toggleEnabled = (item: InstanceContent, next: boolean) =>
    change(
      () => (item.enabled = next),
      () => (item.enabled = !next),
      () => api.setContentEnabled(instance.id, item.id, next),
    );

  const toggleInstalled = (item: InstanceContent, next: boolean) =>
    change(
      () => (item.installed = next),
      () => (item.installed = !next),
      () => api.setOptionalInstalled(instance.id, item.id, next),
    );

  // "Set all shown": every listed file the owner can still set, in small batches so
  // hundreds of files don't flood the backend at once.
  let settingAll = $state(false);
  async function setAllShown(next: ShareMode) {
    const items = visible.filter((c) => c.installed && c.shareMode !== next);
    settingAll = true;
    for (let i = 0; i < items.length; i += 20)
      await Promise.all(items.slice(i, i + 20).map((c) => setMode(c, next)));
    settingAll = false;
  }

  function setMode(item: InstanceContent, next: ShareMode) {
    const before = item.shareMode;
    return change(
      () => (item.shareMode = next),
      () => (item.shareMode = before),
      () => api.setShareMode(instance.id, item.id, next),
    );
  }
</script>

<!-- One row: search, kinds (when there's more than one), why it's locked, then bulk
     setting on the right. -->
<div class="mt-4 mb-2 flex flex-wrap items-center gap-x-2.5 gap-y-2">
  <label
    class="flex h-7.5 w-56 max-w-full items-center gap-2 rounded-md bg-(--bg-raise) px-2.5 text-(--text-faint)"
  >
    <Icon name="search" class="size-3.5" />
    <input
      bind:value={query}
      placeholder="Search {plural(instance.content.length, 'file')}"
      class="h-full w-full bg-transparent text-[13px] text-(--text-primary) outline-none placeholder:text-(--text-faint)"
    />
  </label>
  <KindChips items={instance.content} bind:kind counts />
  <!-- While the game runs, the bar above already says so. -->
  {#if lockedReason && !instance.isRunning}
    <span class="min-w-0 truncate text-[13px] text-(--text-faint)"
      >{lockedReason}</span
    >
  {/if}

  {#if canSetModes && visible.length > 1 && !lockedReason}
    <div class="ml-auto flex items-center gap-1.5">
      <Menu
        label="Set every file shown"
        items={[
          { id: "required", label: "Required" },
          { id: "optional", label: "Optional" },
          { id: "private", label: "Only me" },
        ].map((m) => ({
          label: m.label,
          onselect: () => setAllShown(m.id as ShareMode),
        }))}
        triggerClass="inline-flex h-7.5 items-center gap-1 rounded-md px-2.5 text-[13px] font-medium text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary) aria-expanded:bg-(--bg-hover)"
      >
        {#snippet trigger()}
          {settingAll ? "Setting…" : "Set all shown"}
          <Icon name="chev" class="size-3 rotate-90" />
        {/snippet}
      </Menu>
    </div>
  {/if}
</div>

<div>
  <!-- What each column is, so a version string reads as one. -->
  {#if visible.length}
    <div
      class="grid items-center gap-3.5 border-b border-(--line) px-2 pb-1.5 text-[13px] text-(--text-faint)
        {columns}"
      aria-hidden="true"
    >
      <span></span>
      <span>Name</span>
      <span>Version</span>
      <span class={canSetModes ? "pl-3" : ""}
        >{canSetModes ? "Friends get" : "From"}</span
      >
      <span class="text-right">On</span>
    </div>
  {/if}
  {#each visible as item (item.id)}
    {@const optionalExtra = isOptionalExtra(item)}
    {@const present = item.installed && item.enabled}
    <div
      class="grid items-center gap-3.5 rounded-md px-2 py-2.25 hover:bg-(--bg-hover)
        {columns}"
    >
      <ContentIcon
        src={item.iconUrl}
        seed={item.name}
        class="size-7 rounded-[5px] {present ? '' : 'opacity-40'}"
      />
      <div class="min-w-0 {present ? '' : 'opacity-60'}">
        <b class="block truncate font-medium" title={item.name}>{item.name}</b>
        <small class="block truncate text-[13px] text-(--text-faint)">
          {KIND[item.kind]}
          {#if canSetModes}
            · <span title={SOURCES[item.source].hint}
              >{SOURCES[item.source].label}</span
            >
          {:else if optionalExtra}
            · <span class="text-(--accent)">Optional</span> from {ownerName}{item.installed
              ? ""
              : " · not downloaded"}
          {:else if ownerName}
            · {item.isManaged ? `from ${ownerName}` : "your addition"}
          {/if}
          {#if reportable(item)}
            ·
            <button
              type="button"
              class="hover:text-(--text-primary) hover:underline"
              onclick={() =>
                openDialog({
                  kind: "report",
                  about: {
                    kind: "file",
                    instanceId: instance.id,
                    contentId: item.id,
                    name: item.name,
                  },
                })}>Report</button
            >
          {/if}
        </small>
      </div>
      <span
        class="truncate font-mono text-[13px] text-(--text-faint)"
        title={item.versionLabel ?? undefined}>{item.versionLabel ?? "—"}</span
      >

      {#if canSetModes}
        <span class="flex items-center gap-1.5">
          <!-- Dot = differs from the published version (reaches friends on next publish). -->
          <i
            class="size-1.5 rounded-full {pending(item)
              ? 'bg-(--accent)'
              : 'bg-transparent'}"
            title={!pending(item)
              ? undefined
              : item.enabled
                ? "Not sent yet"
                : "Turned off: removed for everyone with your next update"}
          ></i>
          <ShareModeSelect
            value={item.shareMode}
            label="How {item.name} is shared"
            onchange={(m) => setMode(item, m)}
          />
        </span>
      {:else}
        <span
          title={SOURCES[item.source].hint}
          class="flex min-w-0 items-center gap-1.5 text-[13px] text-(--text-muted)"
        >
          <i class="size-1.5 shrink-0 rounded-xs {SOURCES[item.source].dot}"
          ></i><span class="truncate">{SOURCES[item.source].label}</span>
        </span>
      {/if}

      {#if optionalExtra}
        <!-- For optional extras the switch means "downloaded", not "enabled". -->
        <Switch
          checked={item.installed}
          disabled={!!lockedReason}
          label="{item.installed ? 'Remove' : 'Download'} {item.name}"
          onchange={(next) => toggleInstalled(item, next)}
        />
      {:else}
        <Switch
          checked={item.enabled}
          disabled={!!lockedReason}
          label="{item.enabled ? 'Disable' : 'Enable'} {item.name}"
          onchange={(next) => toggleEnabled(item, next)}
        />
      {/if}
    </div>
  {:else}
    <p class="py-12 text-center text-(--text-muted)">
      {query
        ? `Nothing matches “${query}”`
        : "No mods or packs in this folder yet."}
    </p>
  {/each}
</div>
