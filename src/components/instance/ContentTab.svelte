<script lang="ts">
  import { api } from "$lib/api";
  import type {
    ContentKind,
    ContentSource,
    InstanceContent,
    InstanceDetail,
  } from "$lib/types";
  import BlockArt from "../BlockArt.svelte";
  import Icon from "../Icon.svelte";
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

  const ownerName = $derived(
    instance.sync && instance.sync.role !== "owner"
      ? (instance.sync.owner?.displayName ?? "the owner")
      : null,
  );

  async function toggle(item: InstanceContent, next: boolean) {
    error = null;
    item.enabled = next; // optimistic: the switch moves immediately
    try {
      await api.setContentEnabled(instance.id, item.id, next);
      onchanged();
    } catch (e) {
      item.enabled = !next; // put it back
      error = (e as Error).message;
    }
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

{#if lockedReason}
  <p class="mb-1 text-xs text-(--text-faint)">{lockedReason}</p>
{/if}
{#if error}
  <p class="mb-1 text-xs text-(--danger)" role="alert">{error}</p>
{/if}

<div>
  {#each visible as item (item.id)}
    <div
      class="grid grid-cols-[28px_minmax(0,1fr)_170px_130px_36px] items-center gap-3.5 rounded-md px-2 py-2.25 hover:bg-(--bg-hover)"
    >
      <BlockArt
        seed={item.name}
        class="size-7 rounded-[5px] {item.enabled ? '' : 'opacity-40'}"
      />
      <div class="min-w-0 {item.enabled ? '' : 'opacity-50'}">
        <b class="block truncate font-medium">{item.name}</b>
        <small class="block text-[11.5px] text-(--text-faint)">
          {SINGULAR[item.kind]}
          {#if ownerName}· {item.isManaged
              ? `from ${ownerName}`
              : "your addition"}{/if}
        </small>
      </div>
      <span
        class="truncate font-(family-name:--font-mono) text-xs text-(--text-faint)"
        >{item.versionLabel ?? "—"}</span
      >
      <span class="flex items-center gap-1.5 text-[11.5px] text-(--text-muted)">
        <i class="size-1.5 rounded-[2px] {SOURCES[item.source].dot}"
        ></i>{SOURCES[item.source].label}
      </span>
      <Switch
        checked={item.enabled}
        disabled={!!lockedReason}
        label="{item.enabled ? 'Disable' : 'Enable'} {item.name}"
        onchange={(next) => toggle(item, next)}
      />
    </div>
  {:else}
    <p class="py-12 text-center text-(--text-muted)">
      {query
        ? `Nothing matches “${query}”`
        : "No mods or packs in this folder yet."}
    </p>
  {/each}
</div>
