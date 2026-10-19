<script lang="ts">
  import { blockPixels, kindFor, type BlockKind } from "$lib/pixel";

  let {
    seed,
    kind,
    class: className = "size-6 rounded",
  }: { seed: string; kind?: BlockKind; class?: string } = $props();

  const pixels = $derived(blockPixels(kind ?? kindFor(seed), seed));
</script>

<svg
  viewBox="0 0 8 8"
  class="block shrink-0 {className}"
  shape-rendering="crispEdges"
  aria-hidden="true"
>
  {#each pixels as p (p.x * 8 + p.y)}
    <rect x={p.x} y={p.y} width="1" height="1" fill={p.fill} />
  {/each}
</svg>
