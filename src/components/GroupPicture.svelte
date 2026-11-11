<script lang="ts">
  import { bannerPixels } from "$lib/pixel";

  // A group's picture: a banner made from its id, the same for everyone in it.
  let {
    id,
    class: className = "size-6 rounded-md",
  }: { id: string; class?: string } = $props();

  const pixels = $derived(bannerPixels(id));
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
