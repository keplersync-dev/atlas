<script lang="ts">
  import { fuzzyFilter } from "$lib/fuzzy";
  import {
    blocked,
    buildTree,
    canChooseFirstInstall,
    countShared,
    DEFAULT_RULES,
    folderAt,
    isShared,
    keptAs,
    setFirstInstall,
    setShared,
    type Folder,
  } from "$lib/fileRules";
  import { fileSize, plural } from "$lib/format";
  import type { FileRules, LeftOutFile, ShareFile } from "$lib/types";
  import Icon from "../Icon.svelte";
  import SearchBox from "../SearchBox.svelte";

  // Every file in the instance besides mods and packs, as a table to tick, like the
  // Modrinth App's export. Files that can never be sent (programs, logins) aren't
  // shown. `rules` stays null until the user changes something, so the dialog only
  // sends new rules when there are some.
  let {
    files,
    saved,
    leftOut = [],
    rules = $bindable(null),
  }: {
    files: ShareFile[];
    /** The rules from the last share, until the user changes them. */
    saved: FileRules;
    leftOut?: LeftOutFile[];
    rules: FileRules | null;
  } = $props();

  const current = $derived(rules ?? saved);
  const visible = $derived(files.filter((f) => !blocked(f)));
  const tree = $derived(buildTree(visible));
  const counts = $derived(countShared(tree, current));

  const overLimit = $derived(
    leftOut.filter((f) => f.reason === "over_limit").length,
  );
  const isDefault = $derived(
    current.include.length +
      current.exclude.length +
      current.firstInstall.length ===
      0,
  );

  // ── Where you are, and what's listed ──
  let at = $state("");
  let query = $state("");
  type Sort = "name" | "size" | "modified";
  let sortBy = $state<Sort>("name");
  let asc = $state(true);

  const folder = $derived(folderAt(tree, at));
  /** "config/create/" → "config/", "config/" → "". */
  const parent = $derived(at.replace(/[^/]+\/$/, ""));

  const key = (x: {
    name?: string;
    path: string;
    sizeBytes: number;
    modified: string | null;
  }) =>
    sortBy === "size"
      ? x.sizeBytes
      : sortBy === "modified"
        ? (x.modified ?? "")
        : (x.name ?? x.path.split("/").pop() ?? "").toLowerCase();
  function sorted<
    T extends {
      path: string;
      sizeBytes: number;
      modified: string | null;
      name?: string;
    },
  >(list: T[]) {
    const dir = asc ? 1 : -1;
    return [...list].sort((a, b) => {
      const [x, y] = [key(a), key(b)];
      return (x < y ? -1 : x > y ? 1 : 0) * dir;
    });
  }

  const matches = $derived(
    query.trim()
      ? fuzzyFilter(visible, query, (f) => [
          f.path.split("/").pop(),
          f.path,
        ]).slice(0, 300)
      : [],
  );
  const searching = $derived(!!query.trim());
  const rowFolders = $derived(searching ? [] : sorted(folder.folders));
  // Search results stay best match first; folders follow the chosen sort.
  const rowFiles = $derived(searching ? matches : sorted(folder.files));

  // The header checkbox covers what's listed: this folder, or the search results.
  const scope = $derived(
    searching
      ? {
          total: matches.length,
          shared: matches.filter((f) => isShared(current, f.path, f.fromPack))
            .length,
        }
      : (counts.get(folder.path) ?? { total: 0, shared: 0 }),
  );

  function tickAll() {
    const on = scope.shared < scope.total;
    if (searching) {
      let next = current;
      for (const f of matches) next = setShared(next, f.path, on);
      rules = next;
    } else {
      rules = setShared(current, folder.path, on);
    }
  }

  const tick = (target: string, on: boolean) =>
    (rules = setShared(current, target, on));

  function sortOn(s: Sort) {
    if (sortBy === s) asc = !asc;
    else {
      sortBy = s;
      asc = s === "name";
    }
  }

  const when = (iso: string | null) =>
    iso
      ? new Date(iso).toLocaleDateString(undefined, { dateStyle: "short" })
      : "";

  const ROW =
    "grid grid-cols-[28px_minmax(0,1fr)_72px_92px_104px] items-center gap-2 px-3";
</script>

{#snippet sortHeader(s: Sort, label: string, cls = "")}
  <button
    type="button"
    onclick={() => sortOn(s)}
    class="tap flex items-center gap-1 text-left font-medium hover:text-(--text-primary) {cls}"
  >
    {label}
    {#if sortBy === s}
      <Icon name="chev" class="size-3 {asc ? '-rotate-90' : 'rotate-90'}" />
    {/if}
  </button>
{/snippet}

{#snippet folderRow(f: Folder)}
  {@const c = counts.get(f.path) ?? { total: 0, shared: 0 }}
  <div class="{ROW} border-t border-(--line) py-2 hover:bg-(--bg-hover)">
    <!-- The whole 28 px cell ticks it, not just the 16 px box. -->
    <label class="grid size-7 cursor-pointer place-items-center">
      <input
        type="checkbox"
        checked={c.total > 0 && c.shared === c.total}
        indeterminate={c.shared > 0 && c.shared < c.total}
        aria-label="Share {f.name}"
        onchange={() => tick(f.path, c.shared < c.total)}
        class="size-4 accent-(--accent)"
      />
    </label>
    <button
      type="button"
      onclick={() => (at = f.path)}
      class="flex min-h-7 min-w-0 items-center gap-2.5 text-left"
      title={f.name}
    >
      <Icon name="folder" class="size-4.5 shrink-0 text-(--text-muted)" />
      <span class="truncate">{f.name}</span>
    </button>
    <span class="text-[13px] text-(--text-muted)">{fileSize(f.sizeBytes)}</span>
    <span class="truncate text-[13px] text-(--text-muted)"
      >{when(f.modified)}</span
    >
    <button
      type="button"
      aria-label="Open {f.name}"
      onclick={() => (at = f.path)}
      class="grid size-7 place-items-center justify-self-end rounded text-(--text-muted) hover:bg-(--bg-raise-2) hover:text-(--text-primary)"
    >
      <Icon name="chev" class="size-4" />
    </button>
  </div>
{/snippet}

{#snippet fileRow(f: ShareFile)}
  {@const on = isShared(current, f.path, f.fromPack)}
  {@const kept = keptAs(current, f.path)}
  {@const folder =
    searching && f.path.includes("/")
      ? f.path.slice(0, f.path.lastIndexOf("/"))
      : ""}
  <div class="group {ROW} border-t border-(--line) py-2 hover:bg-(--bg-hover)">
    <label class="grid size-7 cursor-pointer place-items-center">
      <input
        type="checkbox"
        checked={on}
        aria-label="Share {f.path}"
        onchange={() => tick(f.path, !on)}
        class="size-4 accent-(--accent)"
      />
    </label>
    <span class="flex min-w-0 items-center gap-2.5">
      <Icon name="file" class="size-4.5 shrink-0 text-(--text-faint)" />
      <!-- The name first; under it, "Changed" and (searching) its folder, so a
           long path cut at the end never hides the name. -->
      <span class="min-w-0">
        <span class="block truncate" title={f.path}
          >{f.path.split("/").pop()}</span
        >
        {#if (on && f.changed) || folder}
          <small class="block truncate text-[13px] text-(--text-faint)">
            {#if on && f.changed}<span
                class="font-medium text-(--accent)"
                title="Set differently from the modpack's copy, so friends get yours. Mods often change their own settings when the game runs, so not every change is yours."
                >Changed</span
              >{#if folder}{" · "}{/if}{/if}{folder}
          </small>
        {/if}
      </span>
    </span>
    <span class="text-[13px] text-(--text-muted)">{fileSize(f.sizeBytes)}</span>
    <span class="truncate text-[13px] text-(--text-muted)"
      >{when(f.modified)}</span
    >
    <span class="flex h-7 items-center justify-end">
      {#if on && canChooseFirstInstall(f.path)}
        <!-- The badge is the switch: shown when on, and on hover or focus when off. -->
        <button
          type="button"
          aria-pressed={kept === "first_install"}
          onclick={() =>
            (rules = setFirstInstall(
              current,
              f.path,
              kept !== "first_install",
            ))}
          title={kept === "first_install"
            ? "Friends get your copy when they join, then keep their own changes. Click to keep it in sync on every update instead."
            : "Send it once: friends get your copy when they join, then keep their own changes."}
          class="shrink-0 rounded-full px-2 text-[13px] {kept ===
          'first_install'
            ? 'bg-(--accent)/12 text-(--accent) hover:bg-(--accent)/20'
            : 'border border-dashed border-(--line-2) text-(--text-muted) opacity-0 group-hover:opacity-100 hover:text-(--text-primary) focus-visible:opacity-100'}"
          >{kept === "first_install" ? "Sent once" : "Send once"}</button
        >
      {:else if on && kept === "first_install"}
        <span
          class="shrink-0 rounded-full bg-(--accent)/12 px-2 text-[13px] text-(--accent)"
          title="Your settings, keys and video options: friends get your copy when they join, then keep their own."
          >Sent once</span
        >
      {:else if on && kept === "merged"}
        <span
          class="shrink-0 rounded-full bg-(--accent)/12 px-2 text-[13px] text-(--accent)"
          title="Your servers are added to each friend's server list. Theirs stay too."
          >Added to theirs</span
        >
      {/if}
    </span>
  </div>
{/snippet}

{#if visible.length}
  <section class="mt-4 rounded-lg border border-(--line) bg-(--bg-raise)">
    <header class="flex items-center gap-2 px-4 pt-3 pb-2.5">
      <h3 class="mr-auto shrink-0 text-[14px] font-semibold">
        Configs and other files
      </h3>
      <SearchBox
        bind:value={query}
        placeholder="Search files"
        class="w-52 min-w-0"
      />
      {#if !isDefault}
        <button
          type="button"
          onclick={() => (rules = DEFAULT_RULES)}
          title="Go back to what Atlas ticks for you"
          class="flex h-8 shrink-0 items-center gap-1.5 rounded-md border border-(--line-2) px-2.5 text-[13px] text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
        >
          <Icon name="restore" class="size-3.5" />Reset
        </button>
      {/if}
    </header>

    {#if overLimit}
      <p
        class="mx-4 mb-2.5 flex items-center gap-2 text-[13px] text-(--warn)"
        role="alert"
      >
        <Icon name="warn" class="size-4 shrink-0" />
        {plural(overLimit, "file")} won't fit in the 150 MB limit. Untick some to
        make room.
      </p>
    {/if}

    <div role="table" aria-label="Files to share" class="text-[14px]">
      <div
        role="row"
        class="{ROW} border-t border-(--line) py-2 text-[13px] text-(--text-muted)"
      >
        <label class="grid size-7 cursor-pointer place-items-center">
          <input
            type="checkbox"
            checked={scope.total > 0 && scope.shared === scope.total}
            indeterminate={scope.shared > 0 && scope.shared < scope.total}
            aria-label={scope.shared < scope.total ? "Tick all" : "Untick all"}
            title={scope.shared < scope.total ? "Tick all" : "Untick all"}
            onchange={tickAll}
            class="size-4 accent-(--accent)"
          />
        </label>
        {@render sortHeader("name", "Name")}
        {@render sortHeader("size", "Size")}
        {@render sortHeader("modified", "Modified")}
        <span></span>
      </div>
      <!-- As tall as its rows, up to about eight: a short folder leaves no empty space. -->
      <div class="max-h-72 overflow-y-auto">
        {#if at && !searching}
          <button
            type="button"
            onclick={() => (at = parent)}
            class="{ROW} w-full border-t border-(--line) py-2 text-left hover:bg-(--bg-hover)"
          >
            <span></span>
            <span class="flex items-center gap-2.5">
              <Icon name="folder-up" class="size-4.5 text-(--text-muted)" />
              Parent folder
            </span>
          </button>
        {/if}
        {#each rowFolders as f (f.path)}
          {@render folderRow(f)}
        {/each}
        {#each rowFiles as f (f.path)}
          {@render fileRow(f)}
        {/each}
        {#if searching && !matches.length}
          <p class="border-t border-(--line) px-4 py-4 text-(--text-muted)">
            Nothing matches “{query}”.
          </p>
        {/if}
      </div>
    </div>
  </section>
{/if}
