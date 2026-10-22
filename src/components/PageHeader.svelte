<script lang="ts">
  import type { Snippet } from "svelte";

  // The top of a page: its name, a one-line summary, and the page's main action on the
  // right. Home, Shared and Friends all start with one.

  let {
    title,
    subtitle,
    actions,
  }: { title: string; subtitle?: string; actions?: Snippet } = $props();
</script>

<header class="mb-6 flex flex-wrap items-end gap-x-6 gap-y-3">
  <div class="min-w-0 flex-1">
    <h1
      class="font-(family-name:--font-display) text-2xl font-semibold tracking-tight"
    >
      {title}
    </h1>
    {#if subtitle}
      <p class="mt-1.5 text-(--text-muted)">{subtitle}</p>
    {/if}
  </div>
  {#if actions}
    <div class="flex items-center gap-2">{@render actions()}</div>
  {/if}
</header>
