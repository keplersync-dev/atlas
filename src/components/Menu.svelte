<script lang="ts" module>
  // A `module` script runs once for the file, not per component instance.
  // It's where you export types (or constants) that other files import.
  export type MenuItem =
    | { label: string; icon?: string; danger?: boolean; onselect: () => void }
    | "divider";
</script>

<script lang="ts">
  import Icon from "./Icon.svelte";

  let { items, label = "More actions" }: { items: MenuItem[]; label?: string } =
    $props();

  let open = $state(false);
  // bind:this gives us the actual DOM element, so we can tell
  // whether a click happened inside the menu or somewhere else.
  let root: HTMLDivElement;

  function onWindowClick(e: MouseEvent) {
    if (open && !root.contains(e.target as Node)) open = false;
  }

  function choose(item: Exclude<MenuItem, "divider">) {
    open = false;
    item.onselect();
  }
</script>

<!-- Listen on the whole window: any click outside closes the menu, so does Escape. -->
<svelte:window
  onclick={onWindowClick}
  onkeydown={(e) => e.key === "Escape" && (open = false)}
/>

<div class="relative" bind:this={root}>
  <button
    aria-label={label}
    aria-haspopup="menu"
    aria-expanded={open}
    onclick={() => (open = !open)}
    class="grid size-8 place-items-center rounded-md text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
  >
    <Icon name="more" />
  </button>

  {#if open}
    <div
      role="menu"
      class="absolute top-full right-0 z-30 mt-1.5 min-w-52 rounded-lg border border-(--line-2) bg-[#1b1f25] p-1.25 shadow-[0_16px_40px_#0009]"
    >
      {#each items as item, i (i)}
        {#if item === "divider"}
          <hr class="my-1 border-(--line)" />
        {:else}
          <button
            role="menuitem"
            onclick={() => choose(item)}
            class="flex w-full items-center gap-2.5 rounded-[5px] px-2.5 py-1.75 text-left text-[12.5px] hover:bg-(--bg-hover)
              {item.danger ? 'text-(--danger)' : ''}"
          >
            {#if item.icon}<Icon name={item.icon} class="size-4" />{/if}
            {item.label}
          </button>
        {/if}
      {/each}
    </div>
  {/if}
</div>
