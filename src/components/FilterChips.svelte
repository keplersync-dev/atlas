<script lang="ts" generics="K extends string, T">
  // A page's filter pills, each with how many of `items` it keeps. "All" shows its
  // count only with `countAll`.
  let {
    filters,
    items,
    value = $bindable(),
    countAll = false,
    class: className = "",
  }: {
    filters: readonly { key: K; label: string; test: (item: T) => boolean }[];
    items: T[];
    value: K;
    countAll?: boolean;
    class?: string;
  } = $props();
</script>

<div class="flex gap-1.5 {className}" role="radiogroup" aria-label="Filter">
  {#each filters as f (f.key)}
    <button
      role="radio"
      aria-checked={value === f.key}
      onclick={() => (value = f.key)}
      class="h-7 rounded-full px-3 text-[13px] {value === f.key
        ? 'bg-(--accent)/15 font-semibold text-(--accent)'
        : 'text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)'}"
    >
      {f.label}
      {#if countAll || f.key !== "all"}<span class="ml-0.5 opacity-70"
          >{items.filter(f.test).length}</span
        >{/if}
    </button>
  {/each}
</div>
