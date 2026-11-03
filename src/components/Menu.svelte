<script lang="ts" module>
  export type { MenuItem } from "./MenuList.svelte";
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";
  import MenuList, { hasItems, type MenuItem } from "./MenuList.svelte";

  // `trigger` replaces the ⋯ button's content; `align` picks the side the list lines up
  // with. With nothing to choose, there's no button at all.
  let {
    items,
    label = "More actions",
    trigger,
    triggerClass = "grid size-8 place-items-center rounded-md text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)",
    align = "right",
  }: {
    items: MenuItem[];
    label?: string;
    trigger?: Snippet;
    triggerClass?: string;
    align?: "left" | "right";
  } = $props();

  let open = $state(false);
  // Opens upward when there isn't room below (a menu near the bottom of the window).
  let up = $state(false);
  let list = $state<HTMLDivElement>();
  $effect(() => {
    if (!open || !list || !root) return;
    const box = root.getBoundingClientRect();
    up =
      box.bottom + list.offsetHeight + 12 > window.innerHeight &&
      box.top > list.offsetHeight + 12;
  });
  let root = $state<HTMLDivElement>();

  function onWindowClick(e: MouseEvent) {
    if (open && !root?.contains(e.target as Node)) open = false;
  }
</script>

<!-- Listen on the whole window: any click outside closes the menu, so does Escape. -->
<svelte:window
  onclick={onWindowClick}
  onkeydown={(e) => e.key === "Escape" && (open = false)}
/>

{#if hasItems(items)}
  <div class="relative" bind:this={root}>
    <button
      type="button"
      aria-label={label}
      aria-haspopup="menu"
      aria-expanded={open}
      onclick={() => (open = !open)}
      class={triggerClass}
    >
      {#if trigger}{@render trigger()}{:else}<Icon name="more" />{/if}
    </button>

    {#if open}
      <MenuList
        {items}
        bind:list
        onchoose={() => (open = false)}
        class="absolute {align === 'left' ? 'left-0' : 'right-0'} {up
          ? 'bottom-full mb-1.5'
          : 'top-full mt-1.5'}"
      />
    {/if}
  </div>
{/if}
