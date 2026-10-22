<script lang="ts">
  import { fileSize, plural } from "$lib/format";
  import type { ContentKind, OptionalItem } from "$lib/types";

  // Optional files a friend marked as "Optional". Nothing is ticked to begin with:
  // the user opts in. `selected` holds the ticked keys (bind:selected).
  let {
    items,
    selected = $bindable([]),
    title = "Optional extras",
  }: { items: OptionalItem[]; selected: string[]; title?: string } = $props();

  const KIND: Record<ContentKind, string> = {
    mod: "Mod",
    resourcepack: "Resource pack",
    shaderpack: "Shader",
    datapack: "Data pack",
  };

  const chosenBytes = $derived(
    items
      .filter((o) => selected.includes(o.key))
      .reduce((n, o) => n + o.sizeBytes, 0),
  );
</script>

{#if items.length}
  <h3 class="mt-4 mb-0.5 text-xs font-semibold text-(--text-muted)">
    {title} <span class="font-normal text-(--text-faint)">{items.length}</span>
  </h3>
  <p class="mb-1.5 text-xs text-(--text-faint)">
    Only downloaded if you tick them. You can change this later.
  </p>
  <div class="rounded-lg bg-(--bg-raise) px-1.5 py-1">
    {#each items as o (o.key)}
      <label
        class="flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5 hover:bg-(--bg-hover)"
      >
        <input
          type="checkbox"
          bind:group={selected}
          value={o.key}
          class="size-3.75 accent-(--accent)"
        />
        <span class="min-w-0 flex-1">
          <b class="block truncate font-medium">{o.name}</b>
          <small class="text-[11.5px] text-(--text-faint)">{KIND[o.kind]}</small
          >
        </span>
        <span class="text-[11.5px] text-(--text-faint)"
          >{fileSize(o.sizeBytes)}</span
        >
      </label>
    {/each}
  </div>
  {#if selected.length}
    <p class="mt-1.5 text-[11.5px] text-(--text-faint)">
      {plural(selected.length, "extra")} selected · {fileSize(chosenBytes)} more to
      download
    </p>
  {/if}
{/if}
