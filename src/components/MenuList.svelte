<script lang="ts" module>
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

  /** Whether there's anything to choose: a menu of only dividers isn't offered. */
  export const hasItems = (items: MenuItem[]) =>
    items.some((item) => item !== "divider");
</script>

<script lang="ts">
  import Icon from "./Icon.svelte";

  // The list itself, shared by the ⋯ menu (Menu) and right-click menus (ContextMenuHost).
  let {
    items,
    onchoose,
    class: className = "",
    style = "",
    list = $bindable(),
  }: {
    items: MenuItem[];
    /** Called first, so the owner closes the list before the action runs. */
    onchoose: () => void;
    class?: string;
    style?: string;
    list?: HTMLDivElement;
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
</script>

<div
  role="menu"
  bind:this={list}
  {style}
  class="z-30 w-max max-w-80 min-w-52 rounded-lg border border-(--line) bg-(--bg-raise-2) p-1.25 shadow-[0_16px_40px_#0009] {className}"
>
  {#each shown as item, i (i)}
    {#if item === "divider"}
      <hr class="my-1 border-(--line)" />
    {:else}
      <button
        type="button"
        role="menuitem"
        onclick={() => {
          onchoose();
          item.onselect();
        }}
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
