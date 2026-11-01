<script lang="ts">
  import { openDialog } from "$lib/dialogs.svelte";
  import { instances } from "$lib/stores";
  import ContentIcon from "../ContentIcon.svelte";
  import Button from "../Button.svelte";

  // "Share an instance" without opening the instance first: a dropdown of every
  // tracked instance, the ones that can't be shared greyed with why. Picking one opens
  // the Share dialog.

  let { class: className = "" }: { class?: string } = $props();

  const all = $derived(
    instances.list.toSorted((a, b) => a.name.localeCompare(b.name)),
  );
  /** Why this one can't be picked, or null when it can. */
  const blocked = (i: (typeof all)[number]) =>
    i.sync
      ? "Already shared"
      : i.sharedElsewhere
        ? "Shared on another account"
        : i.status === "missing"
          ? "Folder not found"
          : i.status === "syncing"
            ? "Updating"
            : null;

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
    icon="plus"
    class="w-full"
  >
    Share an instance
  </Button>

  {#if open}
    <div
      role="menu"
      class="absolute top-full right-0 z-30 mt-1.5 w-72 rounded-lg border border-(--line) bg-(--bg-raise-2) p-1.25 shadow-[0_16px_40px_#0009]"
    >
      <div class="max-h-80 overflow-y-auto">
        {#each all as i (i.id)}
          {@const why = blocked(i)}
          <button
            role="menuitem"
            disabled={!!why}
            title={why ? `${i.name}: ${why}` : i.name}
            onclick={() => pick(i.id)}
            class="flex w-full items-center gap-2.5 rounded-[5px] px-2 py-1.5 text-left {why
              ? 'opacity-50'
              : 'hover:bg-(--bg-hover)'}"
          >
            <ContentIcon src={i.icon} seed={i.id} class="size-7 rounded-md" />
            <span class="min-w-0 flex-1">
              <b class="block truncate text-[13px] font-medium">{i.name}</b>
              <small class="block truncate text-[13px] text-(--text-faint)"
                >{why ? `${i.launcher.name} · ${why}` : i.launcher.name}</small
              >
            </span>
          </button>
        {:else}
          <p class="px-2.5 py-2 text-[13px] text-(--text-muted)">
            Add an instance from your launcher first, then share it.
          </p>
        {/each}
      </div>
      <hr class="my-1 border-(--line)" />
      <button
        role="menuitem"
        onclick={() => {
          open = false;
          openDialog({ kind: "track" });
        }}
        class="w-full rounded-[5px] px-2.5 py-1.75 text-left text-[13px] text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
      >
        {instances.list.length ? "Add another instance…" : "Add an instance…"}
      </button>
    </div>
  {/if}
</div>
