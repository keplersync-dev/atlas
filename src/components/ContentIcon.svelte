<script lang="ts">
  import BlockArt from "./BlockArt.svelte";

  // A mod's or pack's own icon (from Modrinth, or taken out of the file by Atlas).
  // Falls back to a generated block when there's no icon or it fails to load.

  let {
    src,
    seed,
    class: className = "size-7 rounded-[5px]",
  }: {
    src: string | null;
    /** For the fallback block: the same seed always draws the same block. */
    seed: string;
    class?: string;
  } = $props();

  // Only real image URLs (http(s), Atlas's asset URLs, data:). Icon *keys* like "leaf"
  // aren't images: draw the block.
  const url = $derived(src && /^(https?:|data:image\/)/.test(src) ? src : null);

  let failed = $state(false);
  // Pixel-art icons are tiny (16–32 px): scale them up with hard edges, not blurred.
  let pixelated = $state(false);

  // A different icon (the list re-used this component for another row): try again.
  $effect(() => {
    src;
    failed = false;
    pixelated = false;
  });
</script>

{#if url && !failed}
  <img
    src={url}
    alt=""
    loading="lazy"
    decoding="async"
    draggable="false"
    onerror={() => (failed = true)}
    onload={(e) => (pixelated = (e.currentTarget as HTMLImageElement).naturalWidth <= 32)}
    class="shrink-0 bg-(--bg-raise-2) object-cover {className}"
    style:image-rendering={pixelated ? "pixelated" : "auto"}
  />
{:else}
  <BlockArt {seed} class={className} />
{/if}
