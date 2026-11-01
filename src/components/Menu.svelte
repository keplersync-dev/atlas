<script lang="ts" module>
  // A `module` script runs once for the file, not per component instance.
  // It's where you export types (or constants) that other files import.
  export type MenuItem =
    | {
        label: string;
        /** One line under the label saying what happens. */
        hint?: string;
        icon?: string;
        /** The one important action in the menu (handing a group over, say). */
        accent?: boolean;
        danger?: boolean;
        onselect: () => void;
      }
    | "divider";
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  // `trigger` replaces the ⋯ button's content (a member chip, say); `align` picks the
  // side the list lines up with.
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

  // Dividers only between items: none first, last or twice, whatever was left out.
  const shown = $derived(
    items.filter(
      (item, i) =>
        item !== "divider" ||
        (items.slice(0, i).some((x) => x !== "divider") &&
          items.slice(i + 1).some((x) => x !== "divider") &&
          items[i + 1] !== "divider"),
    ),
  );

  // When some items have an icon, the others keep its space so every label lines up.
  const iconGap = $derived(
    shown.some((item) => item !== "divider" && item.icon),
  );

  let open = $state(false);
  // Opens upward when there isn't room below (a menu near the bottom of the window).
  let up = $state(false);
  let list = $state<HTMLDivElement>();
  $effect(() => {
    if (!open || !list) return;
    up =
      root.getBoundingClientRect().bottom + list.offsetHeight + 12 >
        window.innerHeight &&
      root.getBoundingClientRect().top > list.offsetHeight + 12;
  });
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
    <div
      role="menu"
      bind:this={list}
      class="absolute {align === 'left'
        ? 'left-0'
        : 'right-0'} z-30 w-max max-w-80 min-w-52 {up
        ? 'bottom-full mb-1.5'
        : 'top-full mt-1.5'} rounded-lg border border-(--line) bg-(--bg-raise-2) p-1.25 shadow-[0_16px_40px_#0009]"
    >
      {#each shown as item, i (i)}
        {#if item === "divider"}
          <hr class="my-1 border-(--line)" />
        {:else}
          <button
            type="button"
            role="menuitem"
            onclick={() => choose(item)}
            class="flex w-full items-center gap-2.5 rounded-[5px] px-2.5 py-1.75 text-left text-[13px]
              {item.accent
              ? 'bg-(--accent)/8 font-medium text-(--accent) hover:bg-(--accent)/15'
              : `hover:bg-(--bg-hover) ${item.danger ? 'text-(--danger)' : ''}`}"
          >
            {#if item.icon}<Icon
                name={item.icon}
                class="size-4"
              />{:else if iconGap}<span class="size-4 shrink-0"></span>{/if}
            {#if item.hint}
              <span>
                {item.label}
                <small class="block text-[13px] font-normal text-(--text-faint)"
                  >{item.hint}</small
                >
              </span>
            {:else}
              {item.label}
            {/if}
          </button>
        {/if}
      {/each}
    </div>
  {/if}
</div>
