<script lang="ts">
  import { openDialog } from "$lib/dialogs.svelte";
  import { instances } from "$lib/stores";
  import BlockArt from "../BlockArt.svelte";
  import Button from "../Button.svelte";
  import Icon from "../Icon.svelte";

  // "Share an instance" without opening the instance first: a dropdown of
  // tracked instances that aren't shared yet. Picking one opens the Share dialog.

  let { class: className = "" }: { class?: string } = $props();

  const unshared = $derived(
    instances.list
      .filter((i) => !i.sync && i.status === "ready")
      .toSorted((a, b) => a.name.localeCompare(b.name)),
  );

  let open = $state(false);
  let root: HTMLDivElement;

  function pick(id: string) {
    open = false;
    openDialog({ kind: "share", instanceId: id });
  }
</script>

<svelte:window
  onclick={(e) => open && !root.contains(e.target as Node) && (open = false)}
  onkeydown={(e) => e.key === "Escape" && (open = false)}
/>

<div class="relative {className}" bind:this={root}>
  <Button
    variant="primary"
    aria-haspopup="menu"
    aria-expanded={open}
    onclick={() => (open = !open)}
    class="w-full gap-1.5"
  >
    <Icon name="plus" class="size-3.5" />Share an instance
  </Button>

  {#if open}
    <div
      role="menu"
      class="absolute top-full right-0 z-30 mt-1.5 w-72 rounded-lg border border-(--line-2) bg-[#1b1f25] p-1.25 shadow-[0_16px_40px_#0009]"
    >
      {#each unshared as i (i.id)}
        <button
          role="menuitem"
          onclick={() => pick(i.id)}
          class="flex w-full items-center gap-2.5 rounded-[5px] px-2 py-1.5 text-left hover:bg-(--bg-hover)"
        >
          <BlockArt seed={i.id} class="size-7 rounded-md" />
          <span class="min-w-0 flex-1">
            <b class="block truncate text-[12.5px] font-medium">{i.name}</b>
            <small class="block truncate text-[11px] text-(--text-faint)"
              >{i.launcher.name}</small
            >
          </span>
        </button>
      {:else}
        <p class="px-2.5 py-2 text-[12.5px] text-(--text-muted)">
          Everything you track is already shared.
        </p>
      {/each}
      <hr class="my-1 border-(--line)" />
      <button
        role="menuitem"
        onclick={() => {
          open = false;
          openDialog({ kind: "track" });
        }}
        class="w-full rounded-[5px] px-2.5 py-1.75 text-left text-[12.5px] text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
      >
        Track another instance…
      </button>
    </div>
  {/if}
</div>
