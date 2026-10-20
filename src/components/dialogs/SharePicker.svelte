<script lang="ts" module>
  import type { Id, PublishItem, ShareMode } from "$lib/types";

  // Plain helpers, exported so the Share/Publish dialogs use exactly the same rules.

  /** The setting the user picked in this dialog, or the item's saved one. */
  export const modeOf = (item: PublishItem, modes: Record<Id, ShareMode>) =>
    modes[item.contentId] ?? item.shareMode;

  /** Unknown files that will be shared, so they have to be uploaded. */
  export const uploadsOf = (
    items: PublishItem[],
    modes: Record<Id, ShareMode>,
  ) =>
    items.filter(
      (i) =>
        i.needsUpload && !i.alreadyUploaded && modeOf(i, modes) !== "private",
    );

  export const uploadBytes = (
    items: PublishItem[],
    modes: Record<Id, ShareMode>,
  ) => uploadsOf(items, modes).reduce((n, i) => n + i.sizeBytes, 0);

  /** Files whose sharing differs from the last published version. */
  export function differences(
    items: PublishItem[],
    modes: Record<Id, ShareMode>,
  ) {
    const added: PublishItem[] = [];
    const changed: PublishItem[] = [];
    for (const i of items) {
      const mode = modeOf(i, modes);
      if (i.publishedShareMode === null || i.publishedShareMode === "private") {
        if (mode !== "private") added.push(i);
      } else if (mode !== i.publishedShareMode) {
        changed.push(i);
      }
    }
    return { added, changed };
  }
</script>

<script lang="ts">
  import { untrack } from "svelte";
  import { fileSize, plural } from "$lib/format";
  import type { ContentKind, PublishPlan } from "$lib/types";
  import BlockArt from "../BlockArt.svelte";
  import Icon from "../Icon.svelte";
  import ShareModeSelect, { SHARE_MODES } from "../ShareModeSelect.svelte";

  // The list of files with a Required / Optional / Only me setting each.
  // `modes` only holds what the user changed in this dialog; everything else
  // keeps its saved setting (new files start "required").
  let {
    items,
    storage,
    firstShare,
    modes = $bindable({}),
    rights = $bindable(false),
  }: {
    items: PublishItem[];
    storage: PublishPlan["storage"];
    /** First share: everything is new, so there's no New/All filter. */
    firstShare: boolean;
    modes: Record<Id, ShareMode>;
    rights: boolean;
  } = $props();

  const newItems = $derived(items.filter((i) => i.isNew));
  // Start on "New" when publishing (that's what needs a decision), "All" on a first share.
  // untrack: this only picks the *starting* tab; it shouldn't jump while you work.
  let filter = $state<"new" | "all">(
    untrack(() => (firstShare || !items.some((i) => i.isNew) ? "all" : "new")),
  );
  let query = $state("");

  const KIND: Record<ContentKind, string> = {
    mod: "Mod",
    resourcepack: "Resource pack",
    shaderpack: "Shader",
    datapack: "Data pack",
  };

  const shown = $derived(
    (filter === "new" ? newItems : items).filter((i) =>
      i.name.toLowerCase().includes(query.trim().toLowerCase()),
    ),
  );

  function setMode(item: PublishItem, mode: ShareMode) {
    // Reassign (not mutate) so the parent's bound object updates too.
    modes = { ...modes, [item.contentId]: mode };
  }

  function setAllShown(mode: ShareMode) {
    const next = { ...modes };
    for (const i of shown) next[i.contentId] = mode;
    modes = next;
  }

  const uploads = $derived(uploadsOf(items, modes));
  const adding = $derived(uploadBytes(items, modes));
  const usedAfter = $derived(storage.usedBytes + adding);
  const overQuota = $derived(usedAfter > storage.quotaBytes);
  const tooBig = $derived(
    uploads.filter((i) => i.sizeBytes > storage.maxFileBytes),
  );
</script>

<!-- What the three settings mean, always visible so nobody has to guess. -->
<p class="mt-1 mb-2.5 text-xs text-(--text-faint)">
  {#each SHARE_MODES as m, i (m.id)}
    <b class="font-semibold text-(--text-muted)">{m.label}</b>: {m.hint.toLowerCase()}{i <
    SHARE_MODES.length - 1
      ? " · "
      : "."}
  {/each}
</p>

<div class="mb-1.5 flex flex-wrap items-center gap-2">
  {#if !firstShare}
    <div
      class="inline-flex gap-0.5 rounded-md bg-(--bg) p-0.5"
      role="group"
      aria-label="Show"
    >
      {#each [{ id: "new", label: `New (${newItems.length})` }, { id: "all", label: `All (${items.length})` }] as f (f.id)}
        <button
          type="button"
          aria-pressed={filter === f.id}
          onclick={() => (filter = f.id as "new" | "all")}
          class="rounded px-2.5 py-0.5 text-xs font-medium {filter === f.id
            ? 'bg-(--bg-raise-2) text-(--text-primary)'
            : 'text-(--text-muted)'}"
        >
          {f.label}
        </button>
      {/each}
    </div>
  {/if}
  <label
    class="flex h-7 min-w-40 flex-1 items-center gap-2 rounded-md bg-(--bg) px-2.5 text-(--text-faint)"
  >
    <Icon name="search" class="size-3.5" />
    <input
      bind:value={query}
      placeholder="Search {filter === 'new'
        ? newItems.length
        : items.length} files"
      class="w-full bg-transparent text-xs text-(--text-primary) outline-none placeholder:text-(--text-faint)"
    />
  </label>
  {#if shown.length > 1}
    <span class="flex items-center gap-1 text-[11.5px] text-(--text-faint)">
      Set all shown:
      {#each SHARE_MODES as m (m.id)}
        <button
          type="button"
          class="rounded px-1.5 py-0.5 text-(--accent) hover:bg-(--bg-hover)"
          onclick={() => setAllShown(m.id)}
        >
          {m.label}
        </button>
      {/each}
    </span>
  {/if}
</div>

<div class="max-h-72 overflow-y-auto rounded-lg bg-(--bg-raise)">
  {#each shown as item (item.contentId)}
    {@const mode = modeOf(item, modes)}
    {@const was = item.publishedShareMode}
    <div
      class="flex items-center gap-3 px-3 py-2 not-last:border-b not-last:border-(--line)"
    >
      <BlockArt
        seed={item.name}
        class="size-7 rounded-[5px] {mode === 'private' ? 'opacity-40' : ''}"
      />
      <div class="min-w-0 flex-1">
        <b
          class="block truncate font-medium {mode === 'private'
            ? 'text-(--text-muted)'
            : ''}">{item.name}</b
        >
        <small class="block truncate text-[11.5px] text-(--text-faint)">
          {KIND[item.kind]}
          {#if item.isNew}· <span class="text-(--accent)">new</span>{/if}
          {#if was && was !== "private" && mode !== was}· was {SHARE_MODES.find(
              (m) => m.id === was,
            )?.label}{/if}
          {#if item.needsUpload && !item.alreadyUploaded && mode !== "private"}
            · <span class="text-(--warn)"
              >not on Modrinth or CurseForge, uploads {fileSize(
                item.sizeBytes,
              )}</span
            >
          {/if}
        </small>
      </div>
      <ShareModeSelect
        value={mode}
        label="How {item.name} is shared"
        onchange={(m) => setMode(item, m)}
      />
    </div>
  {:else}
    <p class="px-3 py-6 text-center text-[12.5px] text-(--text-muted)">
      {#if query}
        Nothing matches “{query}”.
      {:else if filter === "new"}
        Nothing new since your last publish. Switch to <b>All</b> to change how existing
        files are shared.
      {:else}
        No files in this instance.
      {/if}
    </p>
  {/each}
</div>

{#if uploads.length}
  <!-- Only shown when an unknown file is set to be shared. -->
  <div class="mt-3 rounded-lg bg-(--bg-raise) px-3.5 py-3">
    <p class="text-[12.5px] text-(--text-muted)">
      {plural(uploads.length, "file")}
      {uploads.length === 1 ? "isn't" : "aren't"} on Modrinth or CurseForge, so
      {uploads.length === 1 ? "it" : "they"} will be uploaded to Atlas for your friends.
      Set
      {uploads.length === 1 ? "it" : "them"} to <b>Only me</b> to keep {uploads.length ===
      1
        ? "it"
        : "them"} local.
    </p>
    <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-(--line-2)">
      <div
        class="h-full {overQuota ? 'bg-(--danger)' : 'bg-(--accent)'}"
        style="width: {Math.min(100, (usedAfter / storage.quotaBytes) * 100)}%"
      ></div>
    </div>
    <div
      class="mt-1.5 flex justify-between text-[11.5px] {overQuota
        ? 'text-(--danger)'
        : 'text-(--text-faint)'}"
    >
      <span
        >{fileSize(usedAfter)} of {fileSize(storage.quotaBytes)} used after upload</span
      >
      {#if overQuota}<span>Not enough space</span>{/if}
    </div>
    {#if tooBig.length}
      <p class="mt-1.5 text-[11.5px] text-(--danger)">
        {tooBig.map((i) => i.name).join(", ")}
        {tooBig.length === 1 ? "is" : "are"} over the {fileSize(
          storage.maxFileBytes,
        )} per-file limit.
      </p>
    {/if}
    <label class="mt-2.5 flex cursor-pointer items-start gap-2.5 text-[12.5px]">
      <input
        type="checkbox"
        bind:checked={rights}
        class="mt-0.5 accent-(--accent)"
      />
      <span>I made these files or have permission to share them.</span>
    </label>
  </div>
{/if}
