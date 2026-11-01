<script lang="ts">
  import { closeContextMenu, contextMenu } from "$lib/contextMenu.svelte";
  import MenuList from "./MenuList.svelte";

  // Draws the right-click menu at the pointer, kept inside the window.
  let list = $state<HTMLDivElement>();
  let at = $state({ x: 0, y: 0 });
  $effect.pre(() => {
    const open = contextMenu.open;
    if (open) at = { x: open.x, y: open.y };
  });
  $effect(() => {
    const open = contextMenu.open;
    if (!open || !list) return;
    const margin = 8;
    at = {
      x: Math.min(open.x, window.innerWidth - list.offsetWidth - margin),
      y:
        open.y + list.offsetHeight + margin > window.innerHeight
          ? Math.max(margin, open.y - list.offsetHeight)
          : open.y,
    };
  });

  // Atlas scrolls inside its panels, not the document: any scroll closes the menu.
  $effect(() => {
    if (!contextMenu.open) return;
    window.addEventListener("scroll", closeContextMenu, true);
    return () => window.removeEventListener("scroll", closeContextMenu, true);
  });

  function onWindowPointer(e: MouseEvent) {
    if (contextMenu.open && !list?.contains(e.target as Node))
      closeContextMenu();
  }
</script>

<svelte:window
  onmousedown={onWindowPointer}
  onkeydown={(e) => e.key === "Escape" && closeContextMenu()}
  onresize={closeContextMenu}
  onblur={closeContextMenu}
/>

{#if contextMenu.open}
  <MenuList
    items={contextMenu.open.items}
    bind:list
    onchoose={closeContextMenu}
    class="fixed z-50"
    style="left: {at.x}px; top: {at.y}px"
  />
{/if}
