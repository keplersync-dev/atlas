<script lang="ts">
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
  import type { FileRules, LeftOutFile, PackInfo, ShareFile } from "$lib/types";
  import Icon from "../Icon.svelte";
  import Menu from "../Menu.svelte";

  // Every file in the instance besides mods and packs, as a table to tick, like the
  // Modrinth App's export. Files that can never be sent (programs, logins) aren't
  // shown. `rules` stays null until the user changes something, so the dialog only
  // sends new rules when there are some.
  let {
    files,
    saved,
    pack = null,
    leftOut = [],
    rules = $bindable(null),
  }: {
    files: ShareFile[];
    /** The rules from the last share, until the user changes them. */
    saved: FileRules;
    pack?: PackInfo | null;
    leftOut?: LeftOutFile[];
    rules: FileRules | null;
  } = $props();

  const current = $derived(rules ?? saved);
  const visible = $derived(files.filter((f) => !blocked(f)));
  const tree = $derived(buildTree(visible));
  const counts = $derived(countShared(tree, current));

  // ── Summary ──
  const shared = $derived(
    visible.filter((f) => isShared(current, f.path, f.fromPack)),
  );
  const fromPack = $derived(shared.filter((f) => f.fromPack).length);
  const uploads = $derived(shared.filter((f) => !f.fromPack));
  const uploadBytes = $derived(uploads.reduce((n, f) => n + f.sizeBytes, 0));
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
  const crumbs = $derived(
    at
      .split("/")
      .filter(Boolean)
      .map((name, i, all) => ({
        name,
        path: all.slice(0, i + 1).join("/") + "/",
      })),
  );

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
      ? visible
          .filter((f) => f.path.toLowerCase().includes(query.toLowerCase()))
          .slice(0, 300)
      : [],
  );
  const searching = $derived(!!query.trim());
  const rowFolders = $derived(searching ? [] : sorted(folder.folders));
  const rowFiles = $derived(sorted(searching ? matches : folder.files));

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
      ? new Date(iso).toLocaleString(undefined, {
          dateStyle: "short",
          timeStyle: "short",
        })
      : "";

  const ROW =
    "grid grid-cols-[28px_minmax(0,1fr)_84px_140px_36px] items-center gap-2 px-3";
</script>

{#snippet sortHeader(s: Sort, label: string, cls = "")}
  <button
    type="button"
    onclick={() => sortOn(s)}
    class="flex items-center gap-1 text-left font-medium hover:text-(--text-primary) {cls}"
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
    <input
      type="checkbox"
      checked={c.total > 0 && c.shared === c.total}
      indeterminate={c.shared > 0 && c.shared < c.total}
      aria-label="Share {f.name}"
      onchange={() => tick(f.path, c.shared < c.total)}
      class="size-4 accent-(--accent)"
    />
    <button
      type="button"
      onclick={() => (at = f.path)}
      class="flex min-w-0 items-center gap-2.5 text-left"
    >
      <Icon name="folder" class="size-4.5 shrink-0 text-(--text-muted)" />
      <span class="truncate">{f.name}</span>
      <span class="shrink-0 text-[13px] text-(--text-faint)"
        >{c.shared} of {c.total}</span
      >
    </button>
    <span class="text-[13px] text-(--text-muted)">{fileSize(f.sizeBytes)}</span>
    <span class="truncate text-[13px] text-(--text-muted)"
      >{when(f.modified)}</span
    >
    <button
      type="button"
      aria-label="Open {f.name}"
      onclick={() => (at = f.path)}
      class="grid size-7 place-items-center rounded text-(--text-muted) hover:bg-(--bg-raise-2) hover:text-(--text-primary)"
    >
      <Icon name="chev" class="size-4" />
    </button>
  </div>
{/snippet}

{#snippet fileRow(f: ShareFile)}
  {@const on = isShared(current, f.path, f.fromPack)}
  {@const kept = keptAs(current, f.path)}
  <div class="{ROW} border-t border-(--line) py-2 hover:bg-(--bg-hover)">
    <input
      type="checkbox"
      checked={on}
      aria-label="Share {f.path}"
      onchange={() => tick(f.path, !on)}
      class="size-4 accent-(--accent)"
    />
    <span class="flex min-w-0 items-center gap-2.5">
      <Icon name="file" class="size-4.5 shrink-0 text-(--text-faint)" />
      <span class="truncate"
        >{searching ? f.path : f.path.split("/").pop()}</span
      >
      {#if on && kept === "first_install"}
        <span
          class="shrink-0 rounded-full bg-(--accent)/12 px-2 text-[12px] text-(--accent)"
          title="Friends get your copy when they join. After that, their own changes stay."
          >Sent once</span
        >
      {:else if on && kept === "merged"}
        <span
          class="shrink-0 rounded-full bg-(--accent)/12 px-2 text-[12px] text-(--accent)"
          title="Your servers are added to each friend's server list. Theirs stay too."
          >Added to theirs</span
        >
      {/if}
    </span>
    <span class="text-[13px] text-(--text-muted)">{fileSize(f.sizeBytes)}</span>
    <span class="truncate text-[13px] text-(--text-muted)"
      >{when(f.modified)}</span
    >
    {#if on && canChooseFirstInstall(f.path)}
      <Menu
        label="Options for {f.path}"
        items={[
          kept === "first_install"
            ? {
                label: "Keep it in sync on every update",
                onselect: () =>
                  (rules = setFirstInstall(current, f.path, false)),
              }
            : {
                label: "Send it once, then friends keep their own",
                onselect: () =>
                  (rules = setFirstInstall(current, f.path, true)),
              },
        ]}
      />
    {:else}
      <span></span>
    {/if}
  </div>
{/snippet}

{#if visible.length}
  <section class="mt-4 rounded-lg border border-(--line) bg-(--bg-raise)">
    <header class="flex items-start gap-3 px-4 pt-3.5">
      <div class="min-w-0 flex-1">
        <h3 class="text-[14px] font-semibold">Other files</h3>
        <p class="text-[13px] text-(--text-muted)">
          Settings, scripts and anything else in the instance. Mods and packs
          are chosen above.
        </p>
      </div>
      {#if !isDefault}
        <button
          type="button"
          onclick={() => (rules = DEFAULT_RULES)}
          class="flex shrink-0 items-center gap-1.5 rounded-md px-2 py-1 text-[13px] text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
        >
          <Icon name="restore" class="size-3.5" />Reset
        </button>
      {/if}
    </header>

    <dl
      class="grid grid-cols-[repeat(auto-fit,minmax(150px,1fr))] gap-2 px-4 pt-3"
    >
      <div class="rounded-md bg-(--bg) px-3 py-2">
        <dt class="text-[12px] text-(--text-muted)">Shared</dt>
        <dd class="text-[14px] font-semibold">
          {shared.length}
          <span class="font-normal text-(--text-muted)"
            >of {plural(visible.length, "file")}</span
          >
        </dd>
      </div>
      {#if pack && fromPack}
        <div class="rounded-md bg-(--bg) px-3 py-2">
          <dt class="truncate text-[12px] text-(--text-muted)">
            From {pack.name}
          </dt>
          <dd class="text-[14px] font-semibold">
            {plural(fromPack, "file")}
            <span class="font-normal text-(--text-muted)">· not uploaded</span>
          </dd>
        </div>
      {/if}
      <div class="rounded-md bg-(--bg) px-3 py-2">
        <dt class="text-[12px] text-(--text-muted)">You upload</dt>
        <dd class="text-[14px] font-semibold">
          {#if uploads.length}
            {plural(uploads.length, "file")}
            <span class="font-normal text-(--text-muted)"
              >· {fileSize(uploadBytes)}</span
            >
          {:else}
            Nothing
          {/if}
        </dd>
      </div>
    </dl>
    {#if overLimit}
      <p
        class="mx-4 mt-2 flex items-center gap-2 text-[13px] text-(--warn)"
        role="alert"
      >
        <Icon name="warn" class="size-4 shrink-0" />
        {plural(overLimit, "file")} won't fit in the 150 MB limit. Untick some to
        make room.
      </p>
    {/if}

    <div class="flex flex-wrap items-center gap-2 px-4 pt-3 pb-2">
      <nav
        aria-label="Folder"
        class="flex min-w-0 flex-1 items-center gap-1 text-[13px]"
      >
        <button
          type="button"
          onclick={() => (at = "")}
          class="rounded px-1 {at
            ? 'text-(--text-muted) hover:text-(--text-primary)'
            : 'font-medium'}">All files</button
        >
        {#each crumbs as c (c.path)}
          <Icon name="chev" class="size-3 shrink-0 text-(--text-faint)" />
          <button
            type="button"
            onclick={() => (at = c.path)}
            class="truncate rounded px-1 {c.path === at
              ? 'font-medium'
              : 'text-(--text-muted) hover:text-(--text-primary)'}"
            >{c.name}</button
          >
        {/each}
      </nav>
      <label class="relative block w-52 max-w-full">
        <span
          class="pointer-events-none absolute top-1/2 left-2.5 flex -translate-y-1/2 text-(--text-faint)"
        >
          <Icon name="search" class="size-3.5" />
        </span>
        <input
          bind:value={query}
          placeholder="Search files"
          aria-label="Search files"
          class="h-8 w-full rounded-md border border-(--line-2) bg-(--bg) pr-2.5 pl-8 text-[13px] outline-none focus:border-(--accent)"
        />
      </label>
    </div>

    <div role="table" aria-label="Files to share" class="text-[13.5px]">
      <div
        role="row"
        class="{ROW} border-t border-(--line) py-2 text-[13px] text-(--text-muted)"
      >
        <input
          type="checkbox"
          checked={scope.total > 0 && scope.shared === scope.total}
          indeterminate={scope.shared > 0 && scope.shared < scope.total}
          aria-label={scope.shared < scope.total ? "Tick all" : "Untick all"}
          title={scope.shared < scope.total ? "Tick all" : "Untick all"}
          onchange={tickAll}
          class="size-4 accent-(--accent)"
        />
        {@render sortHeader("name", "Name")}
        {@render sortHeader("size", "Size")}
        {@render sortHeader("modified", "Modified")}
        <span></span>
      </div>
      <div class="h-72 overflow-y-auto">
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
