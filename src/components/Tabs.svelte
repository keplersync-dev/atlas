<script lang="ts" generics="T extends string">
  // `generics` makes the component typed by its tab ids: bind:value={tab}
  // only accepts the ids you passed in `tabs`.
  //
  // `$bindable()` marks `value` as two-way: the parent writes
  // <Tabs bind:value={tab} …/> and its `tab` variable changes when a tab is clicked.
  let {
    tabs,
    value = $bindable(),
  }: {
    tabs: { id: T; label: string; count?: number | null }[];
    value: T;
  } = $props();
</script>

<div role="tablist" class="flex gap-5.5 border-b border-(--line)">
  {#each tabs as tab (tab.id)}
    {@const selected = value === tab.id}
    <button
      role="tab"
      aria-selected={selected}
      onclick={() => (value = tab.id)}
      class="relative flex items-center gap-1.5 pb-2.5 font-medium transition-colors
        {selected
        ? 'text-(--text-primary)'
        : 'text-(--text-muted) hover:text-(--text-primary)'}"
    >
      {tab.label}
      {#if tab.count != null}
        <span class="font-normal text-(--text-faint) tabular-nums"
          >{tab.count}</span
        >
      {/if}
      {#if selected}
        <span
          class="absolute inset-x-0 -bottom-px h-0.5 rounded-full bg-(--accent)"
        ></span>
      {/if}
    </button>
  {/each}
</div>
