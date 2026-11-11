<script lang="ts" module>
  import type { Id, PublishItem, PublishPlan, ShareMode } from "$lib/types";
  import { modeOf } from "$lib/publishChanges";

  // Plain helpers, exported so the Share/Publish dialogs use exactly the same rules.

  /**
   * The unknown files that will be shared, so they have to be uploaded, and what stops
   * sharing: a file Modrinth and CurseForge couldn't be asked about (it waits for them),
   * or uploads over the storage quota or the per-file limit.
   */
  export function uploadCheck(
    plan: Pick<PublishPlan, "items" | "storage">,
    modes: Record<Id, ShareMode>,
  ) {
    const shared = plan.items.filter((i) => modeOf(i, modes) !== "private");
    const uploads = shared.filter(
      (i) => i.needsUpload && !i.alreadyUploaded && !i.unchecked,
    );
    const unchecked = shared.filter((i) => i.unchecked);
    const usedAfter =
      plan.storage.usedBytes + uploads.reduce((n, i) => n + i.sizeBytes, 0);
    const overQuota = usedAfter > plan.storage.quotaBytes;
    const tooBig = uploads.filter(
      (i) => i.sizeBytes > plan.storage.maxFileBytes,
    );
    return {
      uploads,
      unchecked,
      usedAfter,
      overQuota,
      tooBig,
      blocked: unchecked.length > 0 || overQuota || tooBig.length > 0,
    };
  }

  /** "Global Packs", "Global Packs and Foo", "Global Packs and 2 others". */
  export function names(items: PublishItem[]) {
    if (items.length <= 2) return items.map((i) => i.name).join(" and ");
    return `${items[0].name} and ${items.length - 1} others`;
  }
</script>

<script lang="ts">
  import { fuzzyFilter } from "$lib/fuzzy";
  import { untrack } from "svelte";
  import { fileSize, KIND, plural } from "$lib/format";
  import type { ContentKind } from "$lib/types";
  import ContentIcon from "../ContentIcon.svelte";
  import Icon from "../Icon.svelte";
  import KindChips from "../KindChips.svelte";
  import Menu from "../Menu.svelte";
  import SearchBox from "../SearchBox.svelte";
  import ShareModeSelect, { SHARE_MODES } from "../ShareModeSelect.svelte";

  // The list of files with a Required / Optional / Only me setting each.
  // `modes` only holds what the user changed in this dialog; everything else
  // keeps its saved setting (new files start "required").
  let {
    items,
    storage,
    firstShare,
    modes = $bindable({}),
    onrecheck,
  }: {
    items: PublishItem[];
    storage: PublishPlan["storage"];
    /** First share: everything is new, so there's no New/All filter. */
    firstShare: boolean;
    modes: Record<Id, ShareMode>;
    onrecheck?: () => void;
  } = $props();

  const newItems = $derived(items.filter((i) => i.isNew));
  // Start on "New" when publishing (that's what needs a decision), "All" on a first share.
  // untrack: this only picks the *starting* tab; it shouldn't jump while you work.
  let filter = $state<"new" | "all">(
    untrack(() => (firstShare || !items.some((i) => i.isNew) ? "all" : "new")),
  );
  let query = $state("");
  let kind = $state<ContentKind | "all">("all");

  const ofKind = $derived(
    (filter === "new" ? newItems : items).filter(
      (i) => kind === "all" || i.kind === kind,
    ),
  );
  const shown = $derived(fuzzyFilter(ofKind, query, (i) => [i.name]));
  // Grouped by whether the file has to go to Atlas, not by its setting, so a row stays
  // put when it's set to Only me. The header counts only what will actually upload.
  const toUpload = (i: PublishItem) =>
    i.needsUpload && !i.alreadyUploaded && !i.unchecked;
  const shownUploads = $derived(shown.filter(toUpload));
  const shownRest = $derived(shown.filter((i) => !toUpload(i)));
  const going = $derived(
    shownUploads.filter((i) => modeOf(i, modes) !== "private"),
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

  const { uploads, unchecked, usedAfter, overQuota, tooBig } = $derived(
    uploadCheck({ items, storage }, modes),
  );
  function keepLocal(list: PublishItem[]) {
    const next = { ...modes };
    for (const i of list) next[i.contentId] = "private";
    modes = next;
  }
</script>

<!-- A panel beside the Files one: its own header, search, kinds and Set all shown. -->
<div
  class="flex h-full min-h-0 flex-col rounded-[10px] border border-(--line)/60"
>
  <div class="px-3 pt-2.5">
    <div class="flex items-center gap-2">
      <h3 class="text-[15px] font-semibold">Mods and packs</h3>
      <span class="text-[13px] text-(--text-faint) tabular-nums"
        >{items.length.toLocaleString()}</span
      >
      <div class="ml-auto shrink-0">
        <Menu
          label="Set every mod and pack shown"
          items={SHARE_MODES.map((m) => ({
            label: m.label,
            hint: m.hint,
            onselect: () => setAllShown(m.id),
          }))}
          triggerClass="inline-flex h-7.5 items-center gap-1 rounded-md px-2.5 text-[13px] font-medium text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary) aria-expanded:bg-(--bg-hover)"
        >
          {#snippet trigger()}
            Set all shown <Icon name="chev" class="size-3 rotate-90" />
          {/snippet}
        </Menu>
      </div>
    </div>
    <SearchBox
      bind:value={query}
      placeholder="Search {plural(
        ofKind.length,
        'mod and pack',
        'mods and packs',
      )}"
      class="mt-1.5"
    />
    <div class="mt-2 flex flex-wrap items-center gap-2">
      {#if !firstShare && newItems.length}
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
              class="h-6 rounded px-2.5 text-[13px] font-medium {filter === f.id
                ? 'bg-(--bg-raise-2) text-(--text-primary)'
                : 'text-(--text-muted)'}"
            >
              {f.label}
            </button>
          {/each}
        </div>
      {/if}
      <KindChips {items} bind:kind counts />
    </div>
    {#if unchecked.length}
      <!-- Whether these need uploading isn't known, so sharing waits for an answer. -->
      <div
        class="mb-3 flex items-start gap-3 rounded-lg bg-(--warn)/10 px-3.5 py-3 text-[13px]"
        role="alert"
      >
        <Icon name="warn" class="mt-0.5 size-4 text-(--warn)" />
        <p class="flex-1 text-(--text-muted)">
          Couldn't check <b class="text-(--text-primary)">{names(unchecked)}</b>
          with Modrinth and CurseForge, so Atlas doesn't know yet whether {unchecked.length ===
          1
            ? "it needs"
            : "they need"} uploading. Either site may be down.
        </p>
        <div class="flex shrink-0 flex-col items-end gap-0.5">
          <button
            type="button"
            onclick={onrecheck}
            class="h-6 rounded px-2 font-medium text-(--accent) hover:bg-(--bg-hover)"
            >Try again</button
          >
          <button
            type="button"
            onclick={() => keepLocal(unchecked)}
            class="h-6 rounded px-2 text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
            >Set to Only me</button
          >
        </div>
      </div>
    {/if}

    {#if overQuota || tooBig.length}
      <!-- Fits nowhere as it is: say why, with the fix. The space itself is in the uploads
           header below. -->
      <div
        class="mb-3 flex items-start gap-3 rounded-lg border border-(--danger)/40 bg-(--danger)/6 px-3.5 py-3 text-[13px]"
        role="alert"
      >
        <Icon name="warn" class="mt-0.5 size-4 text-(--danger)" />
        <p class="flex-1 text-(--text-muted)">
          {#if overQuota}
            <b class="text-(--text-primary)">Not enough upload space.</b>
            These uploads need {fileSize(usedAfter - storage.usedBytes)}, and
            you have
            {fileSize(Math.max(0, storage.quotaBytes - storage.usedBytes))} left of
            your
            {fileSize(storage.quotaBytes)}. Free some up from the Shared page,
            or keep them to yourself.
          {:else}
            <b class="text-(--text-primary)">{names(tooBig)}</b>
            {tooBig.length === 1 ? "is" : "are"} over the {fileSize(
              storage.maxFileBytes,
            )} limit for one upload.
          {/if}
        </p>
        <div class="shrink-0">
          <button
            type="button"
            onclick={() => keepLocal(overQuota ? uploads : tooBig)}
            class="h-6 rounded px-2 text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
            >Set to Only me</button
          >
        </div>
      </div>
    {/if}
  </div>

  {#snippet row(item: PublishItem, upload: boolean)}
    {@const mode = modeOf(item, modes)}
    {@const was = item.publishedShareMode}
    <div class="flex items-center gap-3 border-t border-(--line) px-3 py-2">
      <ContentIcon
        src={item.iconUrl}
        seed={item.name}
        class="size-7 rounded-[5px] {mode === 'private' ? 'opacity-40' : ''}"
      />
      <div class="min-w-0 flex-1">
        <b
          class="block truncate font-medium {mode === 'private'
            ? 'text-(--text-muted)'
            : ''}"
          title={item.name}>{item.name}</b
        >
        <small class="block truncate text-[13px] text-(--text-faint)">
          {KIND[item.kind]}
          {#if item.isNew && !firstShare}· <span class="text-(--accent)"
              >new</span
            >{/if}
          {#if was && was !== "private" && mode !== was}· was {SHARE_MODES.find(
              (m) => m.id === was,
            )?.label}{/if}
          {#if upload && mode === "private"}· stays on this PC{/if}
          {#if !item.enabled}· <span class="font-medium text-(--warn)"
              >Off for you</span
            >{/if}
        </small>
        <!-- Its own line, so the warning is never the part that gets cut off. -->
        {#if item.unchecked && mode !== "private"}
          <small class="block text-[13px] text-(--warn)">
            Couldn't check with Modrinth and CurseForge yet
          </small>
        {/if}
      </div>
      {#if upload}
        <span
          class="w-16 shrink-0 text-right text-[13px] text-(--warn) tabular-nums {mode ===
          'private'
            ? 'line-through opacity-60'
            : ''}"
          title={mode === "private"
            ? "Stays on this PC, so nothing uploads"
            : `Uploads ${fileSize(item.sizeBytes)} to Atlas`}
          >{fileSize(item.sizeBytes)}</span
        >
      {/if}
      <ShareModeSelect
        value={mode}
        label="How {item.name} is shared"
        onchange={(m) => setMode(item, m)}
      />
    </div>
  {/snippet}

  <!-- [&>*:first-child]: the top row or header has no line above it. -->
  <div
    class="mt-2 min-h-0 flex-1 overflow-y-auto border-t border-(--line)/60 [&>*:first-child]:border-t-0"
  >
    {#if shownUploads.length}
      <div
        class="flex items-center gap-2 bg-(--warn)/9 px-3 py-2 text-[13px] text-(--warn)"
      >
        <Icon name="upload" class="size-3.5" />
        <span class="font-semibold whitespace-nowrap">Uploads to Atlas</span>
        <span class="min-w-0 truncate text-(--text-faint)"
          >Not on Modrinth or CurseForge</span
        >
        <span
          class="ml-auto flex shrink-0 items-center gap-2 whitespace-nowrap tabular-nums"
        >
          {#if going.length}
            {fileSize(going.reduce((n, i) => n + i.sizeBytes, 0))}
            <!-- How full the space will be; at least a sliver, so it reads as a meter. -->
            <span
              class="h-1 w-11 overflow-hidden rounded-full bg-(--line)"
              aria-hidden="true"
              ><span
                class="block h-full {overQuota
                  ? 'bg-(--danger)'
                  : 'bg-(--accent)'}"
                style="width: max(2px, {Math.min(
                  100,
                  (usedAfter / storage.quotaBytes) * 100,
                )}%)"
              ></span></span
            >
            <span class={overQuota ? "text-(--danger)" : "text-(--text-faint)"}
              >{fileSize(usedAfter)} of {fileSize(storage.quotaBytes)}</span
            >
          {:else}
            Nothing
          {/if}
        </span>
      </div>
      {#each shownUploads as item (item.contentId)}
        {@render row(item, true)}
      {/each}
      {#if shownRest.length}
        <div
          class="flex items-center gap-2 border-t border-(--line) px-3 py-2 text-[13px] text-(--text-faint)"
        >
          <span class="font-semibold">No upload needed</span>
          <span class="ml-auto tabular-nums"
            >{plural(shownRest.length, "file")}</span
          >
        </div>
      {/if}
    {/if}
    {#each shownRest as item (item.contentId)}
      {@render row(item, false)}
    {/each}
    {#if !shown.length}
      <p class="px-3 py-6 text-center text-[13px] text-(--text-muted)">
        {#if query}
          Nothing matches “{query}”.
        {:else if filter === "new"}
          Nothing new since your last update. Switch to <b>All</b> to change how existing
          files are shared.
        {:else}
          No files in this instance.
        {/if}
      </p>
    {/if}
  </div>
</div>
