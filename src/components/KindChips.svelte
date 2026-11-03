<script lang="ts">
  import { KINDS } from "$lib/format";
  import type { ContentKind } from "$lib/types";

  // All, then each kind the list has; nothing when it has only one.
  let {
    items,
    kind = $bindable(),
    counts = false,
    class: className = "",
  }: {
    items: { kind: ContentKind }[];
    kind: ContentKind | "all";
    counts?: boolean;
    class?: string;
  } = $props();

  const chips = $derived(
    (Object.keys(KINDS) as ContentKind[])
      .map((k) => ({
        id: k,
        label: KINDS[k],
        count: items.filter((i) => i.kind === k).length,
      }))
      .filter((c) => c.count > 0),
  );
</script>

{#if chips.length > 1}
  <div class="flex flex-wrap gap-1 {className}" role="group" aria-label="Kind">
    {#each [{ id: "all" as const, label: "All", count: items.length }, ...chips] as chip (chip.id)}
      <button
        type="button"
        aria-pressed={kind === chip.id}
        onclick={() => (kind = chip.id)}
        class="h-7 rounded-full px-2.75 text-[13px] font-medium transition-colors
          {kind === chip.id
          ? 'bg-(--bg-raise-2) text-(--text-primary)'
          : 'text-(--text-muted) hover:bg-(--bg-hover)'}"
      >
        {chip.label}
        {#if counts}<span class="ml-0.5 font-normal opacity-70">{chip.count}</span
          >{/if}
      </button>
    {/each}
  </div>
{/if}
