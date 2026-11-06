<script lang="ts">
  import ContentIcon from "./ContentIcon.svelte";

  // The first few icons of a long list, overlapping: a list that's one row in a dialog
  // still shows what's in it. `ring` is the colour behind, so each icon stands apart.
  let {
    items,
    ring = "var(--bg-raise)",
  }: { items: { src: string | null; seed: string }[]; ring?: string } =
    $props();
</script>

<span class="flex shrink-0" aria-hidden="true">
  {#each items.slice(0, 4) as it, i (i)}
    <span
      class="rounded-md {i ? '-ml-2.5' : ''}"
      style="box-shadow: 0 0 0 2px {ring}"
    >
      <ContentIcon src={it.src} seed={it.seed} class="size-7 rounded-md" />
    </span>
  {/each}
</span>
