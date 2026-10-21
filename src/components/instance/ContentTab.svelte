<script lang="ts">
  import { api } from "$lib/api";
  import type {
    ContentKind,
    ContentSource,
    InstanceContent,
    InstanceDetail,
    ShareMode,
  } from "$lib/types";
  import ContentIcon from "../ContentIcon.svelte";
  import Icon from "../Icon.svelte";
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

  // Local UI state: only this component cares about the filter and search text.
  let kind = $state<ContentKind | "all">("all");
  let query = $state("");
  let error = $state<string | null>(null);

  const KIND_LABELS: Record<ContentKind, string> = {
    mod: "Mods",
    resourcepack: "Resource packs",
    shaderpack: "Shaders",
    datapack: "Data packs",
  };
  const SINGULAR: Record<ContentKind, string> = {
    mod: "Mod",
    resourcepack: "Resource pack",
    shaderpack: "Shader",
    datapack: "Data pack",
  };
  const SOURCES: Record<ContentSource, { label: string; dot: string }> = {
    modrinth: { label: "Modrinth", dot: "bg-[#1bd96a]" },
    curseforge: { label: "CurseForge", dot: "bg-[#f16436]" },
    upload: { label: "Uploaded to Atlas", dot: "bg-(--accent)" },
    local: { label: "Only on this PC", dot: "border border-(--text-faint)" },
  };

  // Owners and editors decide how each file is shared; viewers see what was decided.
  const canSetModes = $derived(
    !!instance.sync && instance.sync.role !== "viewer",
  );
  const ownerName = $derived(
    instance.sync && instance.sync.role === "viewer"
      ? (instance.sync.owner?.displayName ?? "the owner")
      : null,
  );

  // Only show chips for kinds this instance actually has.
  const chips = $derived(
    (Object.keys(KIND_LABELS) as ContentKind[])
      .map((k) => ({
        id: k,
        label: KIND_LABELS[k],
        count: instance.content.filter((c) => c.kind === k).length,
      }))
      .filter((c) => c.count > 0),
  );

  const visible = $derived(
    instance.content.filter(
      (c) =>
        (kind === "all" || c.kind === kind) &&
        c.name.toLowerCase().includes(query.trim().toLowerCase()),
    ),
  );

  /** Owners/editors: has this file's sharing changed since the last publish? */
  const unpublished = (c: InstanceContent) =>
    c.publishedShareMode === null
      ? c.shareMode !== "private"
      : c.shareMode !== c.publishedShareMode;

  /** Viewers: optional items from the shared version can be downloaded or removed freely. */
  const isOptionalExtra = (c: InstanceContent) =>
    !canSetModes && c.isManaged && c.shareMode === "optional";

  /** Optimistic change: update the row now, undo it if the API refuses. */
  async function change(
    apply: () => void,
    undo: () => void,
    call: () => Promise<unknown>,
  ) {
    error = null;
    apply();
    try {
      await call();
      onchanged();
    } catch (e) {
      undo();
      error = (e as Error).message;
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

  function setMode(item: InstanceContent, next: ShareMode) {
    const before = item.shareMode;
    change(
      () => (item.shareMode = next),
      () => (item.shareMode = before),
      () => api.setShareMode(instance.id, item.id, next),
    );
  }
</script>

<div class="mt-4 mb-1.5 flex items-center gap-2.5">
  <div class="flex gap-1">
    {#each [{ id: "all" as const, label: "All", count: instance.content.length }, ...chips] as chip (chip.id)}
      <button
        aria-pressed={kind === chip.id}
        onclick={() => (kind = chip.id)}
        class="h-7 rounded-full px-2.75 text-xs font-medium transition-colors
          {kind === chip.id
          ? 'bg-(--bg-raise-2) text-(--text-primary)'
          : 'text-(--text-muted) hover:bg-(--bg-hover)'}"
      >
        {chip.label}
      </button>
    {/each}
  </div>

  <label
    class="ml-auto flex h-7.5 w-56 items-center gap-2 rounded-md bg-(--bg-raise) px-2.5 text-(--text-faint)"
  >
    <Icon name="search" class="size-3.5" />
    <!-- bind:value keeps `query` and the input in sync both ways -->
    <input
      bind:value={query}
      placeholder="Filter {instance.content.length} items"
      class="w-full bg-transparent text-[12.5px] text-(--text-primary) outline-none placeholder:text-(--text-faint)"
    />
  </label>
</div>

{#if canSetModes}
  <p class="mb-1 text-xs text-(--text-faint)">
    Choose how each file is shared. Changes reach friends when you publish.
  </p>
{/if}
{#if lockedReason}
  <p class="mb-1 text-xs text-(--text-faint)">{lockedReason}</p>
{/if}
{#if error}
  <p class="mb-1 text-xs text-(--danger)" role="alert">{error}</p>
{/if}

<div>
  {#each visible as item (item.id)}
    {@const optionalExtra = isOptionalExtra(item)}
    {@const present = item.installed && item.enabled}
    <div
      class="grid items-center gap-3.5 rounded-md px-2 py-2.25 hover:bg-(--bg-hover)
        {canSetModes
        ? 'grid-cols-[28px_minmax(0,1fr)_110px_auto_36px]'
        : 'grid-cols-[28px_minmax(0,1fr)_150px_130px_36px]'}"
    >
      <ContentIcon
        src={item.iconUrl}
        seed={item.name}
        class="size-7 rounded-[5px] {present ? '' : 'opacity-40'}"
      />
      <div class="min-w-0 {present ? '' : 'opacity-60'}">
        <b class="block truncate font-medium">{item.name}</b>
        <small class="block truncate text-[11.5px] text-(--text-faint)">
          {SINGULAR[item.kind]}
          {#if canSetModes}
            · {SOURCES[item.source].label}
          {:else if optionalExtra}
            · <span class="text-(--accent)">Optional</span> from {ownerName}{item.installed
              ? ""
              : " · not downloaded"}
          {:else if ownerName}
            · {item.isManaged ? `from ${ownerName}` : "your addition"}
          {/if}
        </small>
      </div>
      <span class="truncate font-mono text-xs text-(--text-faint)"
        >{item.versionLabel ?? "—"}</span
      >

      {#if canSetModes}
        <span class="flex items-center gap-1.5">
          <!-- Dot = differs from the published version (reaches friends on next publish). -->
          <i
            class="size-1.5 rounded-full {unpublished(item)
              ? 'bg-(--accent)'
              : 'bg-transparent'}"
            title={unpublished(item) ? "Not published yet" : undefined}
          ></i>
          <ShareModeSelect
            value={item.shareMode}
            label="How {item.name} is shared"
            onchange={(m) => setMode(item, m)}
          />
        </span>
      {:else}
        <span
          class="flex items-center gap-1.5 text-[11.5px] text-(--text-muted)"
        >
          <i class="size-1.5 rounded-xs {SOURCES[item.source].dot}"
          ></i>{SOURCES[item.source].label}
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
