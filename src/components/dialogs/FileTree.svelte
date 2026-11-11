<script lang="ts">
  import { untrack } from "svelte";
  import { fuzzyFilter } from "$lib/fuzzy";
  import {
    blocked,
    buildTree,
    choices,
    countShared,
    DEFAULT_RULES,
    folderAt,
    keepOf,
    setKeep,
    setShared,
    type Folder,
    type Keep,
  } from "$lib/fileRules";
  import { plural } from "$lib/format";
  import { sameRules } from "$lib/publishChanges";
  import type { FileRules, LeftOutFile, ShareFile } from "$lib/types";
  import Button from "../Button.svelte";
  import Icon from "../Icon.svelte";
  import Menu, { type MenuItem } from "../Menu.svelte";
  import SearchBox from "../SearchBox.svelte";

  // Every file in the instance besides mods and packs, one folder at a time, each with
  // the same "Friends get" switch as mods. Files that can never be sent (programs,
  // logins) aren't shown. `rules` stays null until the user changes something, so the
  // dialog only sends new rules when there are some.
  let {
    files,
    saved,
    leftOut = [],
    rules = $bindable(null),
    pending = null,
    onsend,
  }: {
    files: ShareFile[];
    /** The rules from the last share, until the user changes them. */
    saved: FileRules;
    leftOut?: LeftOutFile[];
    rules: FileRules | null;
    /** Paths friends don't have as they are here yet, in the Files tab: they get a dot,
     *  and the list grows with the page. Null in the share dialog. */
    pending?: string[] | null;
    /** The Files tab: send an update with what has a dot. */
    onsend?: () => void;
  } = $props();

  const current = $derived(rules ?? saved);
  const visible = $derived(files.filter((f) => !blocked(f)));
  const tree = $derived(buildTree(visible));
  const counts = $derived(countShared(tree, current));
  const page = $derived(pending !== null);

  const overLimit = $derived(
    leftOut.filter((f) => f.reason === "over_limit").length,
  );
  // By what friends would get, not by the saved lists: every instance starts with an
  // old "config/" rule that shares just what Atlas shares anyway.
  const isDefault = $derived(sameRules(current, DEFAULT_RULES, visible));

  // Not sent yet: what the plan said when the tab opened, plus whatever changed since.
  // shortcut: a file set back to what friends have keeps its dot until the tab reopens.
  const opened = untrack(() => saved);
  const notSent = $derived(
    pending === null
      ? new Set<string>()
      : new Set(
          visible
            .filter(
              (f) =>
                pending.includes(f.path) ||
                keepOf(opened, f) !== keepOf(current, f),
            )
            .map((f) => f.path),
        ),
  );
  const notSentIn = (path: string) =>
    [...notSent].some((p) => p.startsWith(path));

  /** Files set differently from the modpack, per folder path ("" for all). */
  const changedIn = $derived.by(() => {
    const m = new Map<string, number>();
    for (const f of visible.filter((f) => f.changed)) {
      const parts = f.path.split("/");
      for (let i = 0; i < parts.length; i++) {
        const p = i ? parts.slice(0, i).join("/") + "/" : "";
        m.set(p, (m.get(p) ?? 0) + 1);
      }
    }
    return m;
  });

  // ── Where you are, and what's listed ──
  let at = $state("");
  let query = $state("");
  let filter = $state<"all" | "changed" | "me">("all");

  const folder = $derived(folderAt(tree, at));
  const crumbs = $derived(at.split("/").filter(Boolean));
  const up = (n: number) =>
    crumbs.length > n ? crumbs.slice(0, n).join("/") + (n ? "/" : "") : at;

  const flat = $derived(!!query.trim() || filter !== "all");

  // A new folder, filter or search starts at the top of its list, not wherever the last
  // one was scrolled to.
  let section = $state<HTMLElement>();
  let list = $state<HTMLElement>();
  $effect(() => {
    void [at, filter, flat];
    untrack(() => {
      if (list) list.scrollTop = 0;
      const top = section?.closest("main")?.getBoundingClientRect().top ?? 0;
      if (page && section && section.getBoundingClientRect().top < top)
        section.scrollIntoView({ block: "start" });
    });
  });
  const shown = $derived.by(() => {
    if (!flat) return [];
    const q = query.trim();
    const found = q
      ? fuzzyFilter(visible, q, (f) => [f.path.split("/").pop(), f.path])
      : visible;
    return found
      .filter((f) =>
        filter === "changed"
          ? f.changed
          : filter === "me"
            ? keepOf(current, f) === "me"
            : true,
      )
      .slice(0, 300);
  });
  const byName = (a: { path: string }, b: { path: string }) =>
    a.path.localeCompare(b.path, undefined, { sensitivity: "base" });
  const rowFolders = $derived(flat ? [] : [...folder.folders].sort(byName));
  const rowFiles = $derived(flat ? shown : [...folder.files].sort(byName));

  const changedCount = $derived(changedIn.get("") ?? 0);
  const onlyMeCount = $derived(
    visible.filter((f) => keepOf(current, f) === "me").length,
  );

  /** Files kept to you by default (map data, bookmarks…), per folder path. */
  const heldIn = $derived.by(() => {
    const m = new Map<string, number>();
    for (const f of visible) {
      if (f.personal?.keep !== "only_me" || keepOf(current, f) !== "me")
        continue;
      const parts = f.path.split("/");
      for (let i = 0; i < parts.length; i++) {
        const p = i ? parts.slice(0, i).join("/") + "/" : "";
        m.set(p, (m.get(p) ?? 0) + 1);
      }
    }
    return m;
  });

  /** "always", "me", or "some" for a folder. Files kept to you by default don't count,
   *  so `config` reads Always while its bookmarks stay yours (they say so on their row). */
  function folderKeep(path: string) {
    const c = counts.get(path) ?? { total: 0, shared: 0 };
    if (c.shared === 0) return "me";
    return c.shared >= c.total - (heldIn.get(path) ?? 0) ? "always" : "some";
  }

  const set = (f: ShareFile, to: Keep) => (rules = setKeep(current, f, to));
  const setFolder = (path: string, to: Keep) =>
    (rules = setShared(current, path, to !== "me"));

  function setAllShown(to: Keep) {
    let next = current;
    for (const fo of rowFolders) next = setShared(next, fo.path, to !== "me");
    for (const f of rowFiles) {
      const can = choices(f.path);
      next = setKeep(next, f, can.includes(to) ? to : can[0]);
    }
    rules = next;
  }

  const KEEPS: Record<Keep, { label: string; hint: string; on: string }> = {
    always: {
      label: "Always",
      hint: "Friends get your copy with every update",
      on: "bg-(--bg-raise-2) text-(--text-primary)",
    },
    once: {
      label: "Once",
      hint: "Friends get your copy when they join, then keep their own",
      on: "bg-(--accent)/15 text-(--accent)",
    },
    me: {
      label: "Only me",
      hint: "Stays on your PC",
      on: "bg-(--bg-raise-2) text-(--text-muted)",
    },
  };

  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

  /** The line under a file's name: why it's kept the way it is, or what changing it does. */
  function line(f: ShareFile, k: Keep): { text: string; warn: boolean } | null {
    if (f.path === "servers.dat")
      return k === "me"
        ? null
        : {
            text: "Your servers show up in friends' Multiplayer list. Theirs stay.",
            warn: false,
          };
    const p = f.personal;
    if (!p) return null;
    if (p.keep === "once")
      return k === "once"
        ? {
            text: `${cap(p.what)}, sent once so friends keep their own`,
            warn: false,
          }
        : k === "always"
          ? {
              text: `Replaces friends' own ${p.what} on every update`,
              warn: true,
            }
          : null;
    return k === "me"
      ? { text: `${cap(p.what)}, kept on your PC`, warn: false }
      : { text: `Sends ${p.what} to friends`, warn: true };
  }

  /** A folder that is itself on the personal list ("journeymap/data/") says so too. */
  const folderLine = (fo: Folder) => {
    const p = visible.find((f) => f.personal?.rule === fo.path)?.personal;
    return p && folderKeep(fo.path) === "me"
      ? `${cap(p.what)}, kept on your PC`
      : null;
  };

  const menu = $derived<MenuItem[]>([
    { label: "Always", onselect: () => setAllShown("always") },
    { label: "Only me", onselect: () => setAllShown("me") },
    ...(isDefault
      ? []
      : [
          "divider" as const,
          {
            label: "Atlas's defaults",
            hint: "What Atlas picks for every file",
            onselect: () => (rules = DEFAULT_RULES),
          },
        ]),
  ]);

  const GRID =
    "grid grid-cols-[16px_minmax(0,1fr)_6px_auto_24px] items-center gap-2.5 px-2";
  const ROW = `${GRID} min-h-11 rounded-md py-1 hover:bg-(--bg-hover)`;
</script>

{#snippet keepSwitch(
  value: Keep | "some",
  options: Keep[],
  onchange: (to: Keep) => void,
  label: string,
)}
  <div
    role="radiogroup"
    aria-label={label}
    class="inline-flex shrink-0 gap-0.5 justify-self-end rounded-md bg-(--bg) p-0.5"
  >
    {#each options as k (k)}
      <button
        type="button"
        role="radio"
        aria-checked={value === k}
        title={KEEPS[k].hint}
        onclick={() => value !== k && onchange(k)}
        class="h-6 rounded px-2 text-[13px] font-medium whitespace-nowrap transition-colors {value ===
        k
          ? KEEPS[k].on
          : 'text-(--text-faint) hover:text-(--text-primary)'}"
        >{KEEPS[k].label}</button
      >
    {/each}
  </div>
{/snippet}

<!-- What each column is; the rows say it again to screen readers. -->
{#snippet columns()}
  <div
    aria-hidden="true"
    class="{GRID} border-b border-(--line) pb-1.5 text-[13px] text-(--text-faint) {page
      ? ''
      : 'sticky top-0 z-10 bg-(--bg-raise)'}"
  >
    <span></span>
    <span>Name</span>
    <span></span>
    <span class="justify-self-end">Friends get</span>
    <span></span>
  </div>
{/snippet}

{#snippet dot(on: boolean)}
  <i
    class="size-1.5 rounded-full {on ? 'bg-(--accent)' : 'bg-transparent'}"
    title={on ? "Friends get this with your next update" : undefined}
  ></i>
{/snippet}

{#if onsend && notSent.size}
  <!-- The pack's change count is mods only, so the header has no Send update for files. -->
  <div
    class="mt-4 flex items-center gap-3.5 rounded-lg bg-(--bg-raise) py-3 pr-3.5 pl-4"
  >
    <Icon name="refresh" class="size-4.5 text-(--text-faint)" />
    <div class="min-w-0 flex-1">
      <b class="font-semibold">{plural(notSent.size, "file")} not sent yet</b>
      <small class="block text-[13px] text-(--text-muted)"
        >Friends get {notSent.size === 1 ? "it" : "them"} with your next update.</small
      >
    </div>
    <Button variant="primary" onclick={onsend}>Send update</Button>
  </div>
{/if}

{#if visible.length}
  <section
    bind:this={section}
    class={page
      ? "mt-4"
      : "mt-4 rounded-lg border border-(--line) bg-(--bg-raise) pt-3"}
  >
    {#if !page}
      <h3 class="px-4 pb-2 text-[14px] font-semibold">
        Configs and other files
      </h3>
    {/if}
    <div class={page ? "sticky top-0 z-10 bg-(--bg) pt-1" : ""}>
      <div
        class="flex flex-wrap items-center gap-x-2 gap-y-2 {page ? '' : 'px-4'}"
      >
        <SearchBox
          bind:value={query}
          placeholder="Search {plural(visible.length, 'file')}"
          class="max-w-72 min-w-32 flex-1"
        />
        {#if page}
          <div class="flex gap-0.5" role="group" aria-label="Show">
            {#each [{ id: "all" as const, label: "All", n: null }, ...(changedCount ? [{ id: "changed" as const, label: "Changed", n: changedCount }] : []), { id: "me" as const, label: "Only me", n: onlyMeCount }] as c (c.id)}
              <button
                type="button"
                aria-pressed={filter === c.id}
                onclick={() => (filter = c.id)}
                class="h-7 rounded-full px-3 text-[13px] {filter === c.id
                  ? 'bg-(--bg-raise-2) text-(--text-primary)'
                  : 'text-(--text-muted) hover:text-(--text-primary)'}"
                >{c.label}{#if c.n !== null}<span
                    class="ml-1 text-(--text-faint) tabular-nums">{c.n}</span
                  >{/if}</button
              >
            {/each}
          </div>
          <!-- A menu, as on the Content tab beside it. -->
          <div class="ml-auto shrink-0">
            <Menu
              label="Set every file shown"
              items={menu}
              triggerClass="inline-flex h-7.5 items-center gap-1 rounded-md px-2.5 text-[13px] font-medium text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary) aria-expanded:bg-(--bg-hover)"
            >
              {#snippet trigger()}
                Set all shown <Icon name="chev" class="size-3 rotate-90" />
              {/snippet}
            </Menu>
          </div>
        {:else}
          <!-- Links, as in the mods list just above it in the dialog. The filters stay
               in the Files tab: there's no room for both here. -->
          <span class="flex items-center gap-1 text-[13px] text-(--text-faint)">
            Set all shown:
            {#each [{ label: "Always", to: "always" as const }, { label: "Only me", to: "me" as const }] as s (s.to)}
              <button
                type="button"
                class="h-6 rounded px-1.5 text-(--accent) hover:bg-(--bg-hover)"
                onclick={() => setAllShown(s.to)}>{s.label}</button
              >
            {/each}
            {#if !isDefault}
              <button
                type="button"
                class="h-6 rounded px-1.5 text-(--accent) hover:bg-(--bg-hover)"
                title="What Atlas picks for every file"
                onclick={() => (rules = DEFAULT_RULES)}>Atlas's defaults</button
              >
            {/if}
          </span>
        {/if}
      </div>

      {#if overLimit}
        <p
          class="mt-2.5 flex items-center gap-2 text-[13px] text-(--warn) {page
            ? ''
            : 'px-4'}"
          role="alert"
        >
          <Icon name="warn" class="size-4 shrink-0" />
          {plural(overLimit, "file")} won't fit in the 150 MB limit. Set some to Only
          me to make room.
        </p>
      {/if}

      <!-- Where you are: a path to go back up, and the open folder's own switch. -->
      <div
        class="mt-3 flex min-h-11 items-center gap-2.5 {page ? 'px-2' : 'px-4'}"
      >
        {#if flat}
          <div class="min-w-0 flex-1">
            <b class="block font-medium"
              >{query.trim()
                ? `Matching “${query.trim()}”`
                : filter === "changed"
                  ? "Changed from the modpack"
                  : "Only me"}</b
            >
            <small class="block text-[13px] text-(--text-faint)"
              >Every folder, as one list{shown.length === 300
                ? ", the first 300"
                : ""}</small
            >
          </div>
        {:else}
          {#if at}
            <button
              type="button"
              aria-label="Back to {crumbs.length > 1
                ? crumbs.at(-2)
                : 'all files'}"
              onclick={() => (at = up(crumbs.length - 1))}
              class="inline-flex h-7 shrink-0 items-center gap-1 rounded-md border border-(--line-2) pr-2.5 pl-1.5 text-[13px] text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
            >
              <Icon name="chev" class="size-3.5 rotate-180" />Back
            </button>
          {/if}
          <div class="min-w-0 flex-1">
            <nav
              aria-label="Folder"
              class="flex min-w-0 items-center gap-1 truncate"
            >
              {#if at}
                <button
                  type="button"
                  class="shrink-0 text-(--accent) hover:underline"
                  onclick={() => (at = "")}>All files</button
                >
                {#each crumbs as c, i (i)}
                  <span class="text-(--text-faint)" aria-hidden="true">›</span>
                  {#if i < crumbs.length - 1}
                    <button
                      type="button"
                      class="truncate text-(--accent) hover:underline"
                      onclick={() => (at = up(i + 1))}>{c}</button
                    >
                  {:else}
                    <b class="truncate font-medium" aria-current="location"
                      >{c}</b
                    >
                  {/if}
                {/each}
              {:else}
                <b class="font-medium">All files</b>
              {/if}
            </nav>
            <small class="block truncate text-[13px] text-(--text-faint)">
              {plural(
                counts.get(at)?.total ?? 0,
                "file",
              )}{#if changedIn.get(at)}{" · "}<span class="text-(--accent)"
                  >{changedIn.get(at)} changed</span
                >{/if}{at && folderKeep(at) === "some"
                ? " · some only you"
                : ""}
            </small>
          </div>
          {#if at}
            <span class="text-[13px] text-(--text-faint)">Whole folder</span>
            {@render keepSwitch(
              folderKeep(at),
              ["always", "me"],
              (to) => setFolder(at, to),
              `What friends get of ${crumbs.at(-1)}`,
            )}
            <!-- Lines the switch up with the rows', which have a chevron after them. -->
            <span class="w-6 shrink-0"></span>
          {/if}
        {/if}
      </div>

      {#if page}{@render columns()}{/if}
    </div>

    <div role="list" aria-label="Files to share">
      <div
        bind:this={list}
        class={page ? "pt-1" : "max-h-72 overflow-y-auto px-2 pb-1"}
      >
        <!-- In the dialog the labels scroll with the rows' box, so they line up with the
             rows whether or not a scrollbar takes some of its width. -->
        {#if !page}{@render columns()}{/if}
        {#each rowFolders as fo (fo.path)}
          {@const k = folderKeep(fo.path)}
          {@const changed = changedIn.get(fo.path) ?? 0}
          {@const note = folderLine(fo)}
          <div role="listitem" class={ROW}>
            <Icon name="folder" class="size-4 text-(--text-muted)" />
            <button
              type="button"
              class="min-w-0 text-left"
              onclick={() => (at = fo.path)}
            >
              <span class="block truncate">{fo.name}</span>
              <small class="block truncate text-[13px] text-(--text-faint)">
                {plural(
                  counts.get(fo.path)?.total ?? 0,
                  "file",
                )}{#if changed}{" · "}<span class="text-(--accent)"
                    >{changed} changed</span
                  >{/if}{k === "some" ? " · some only you" : ""}{note
                  ? ` · ${note}`
                  : ""}
              </small>
            </button>
            {@render dot(notSentIn(fo.path))}
            {@render keepSwitch(
              k,
              ["always", "me"],
              (to) => setFolder(fo.path, to),
              `What friends get of ${fo.name}`,
            )}
            <button
              type="button"
              aria-label="Open {fo.name}"
              onclick={() => (at = fo.path)}
              class="grid size-6 place-items-center rounded text-(--text-muted) hover:bg-(--bg-raise-2) hover:text-(--text-primary)"
            >
              <Icon name="chev" class="size-4" />
            </button>
          </div>
        {/each}
        {#each rowFiles as f (f.path)}
          {@const k = keepOf(current, f)}
          {@const why = line(f, k)}
          {@const name = f.path.split("/").pop()}
          {@const where =
            flat && f.path.includes("/")
              ? f.path.slice(0, f.path.lastIndexOf("/"))
              : ""}
          <div role="listitem" class={ROW}>
            <Icon name="file" class="size-4 text-(--text-faint)" />
            <span class="min-w-0">
              <span class="block truncate" title={f.path}
                >{#if f.path === "servers.dat"}Server list <span
                    class="text-[13px] text-(--text-faint)">servers.dat</span
                  >{:else}{name}{/if}</span
              >
              {#if (f.changed && k !== "me") || why || where}
                <small class="block truncate text-[13px] text-(--text-faint)">
                  {#if f.changed && k !== "me"}<span
                      class="text-(--accent)"
                      title="Set differently from the modpack's copy, so friends get yours. Mods often change their own settings when the game runs, so not every change is yours."
                      >Changed from the modpack</span
                    >{:else if why}<span class={why.warn ? "text-(--warn)" : ""}
                      >{why.text}</span
                    >{/if}{#if where}{(f.changed && k !== "me") || why
                      ? " · "
                      : ""}{where}{/if}
                </small>
              {/if}
            </span>
            {@render dot(notSent.has(f.path))}
            {@render keepSwitch(
              k,
              choices(f.path),
              (to) => set(f, to),
              `What friends get of ${name}`,
            )}
            <span></span>
          </div>
        {/each}
        {#if !rowFolders.length && !rowFiles.length}
          <p class="px-2 py-5 text-[13px] text-(--text-muted)">
            {query.trim()
              ? `Nothing matches “${query.trim()}”.`
              : filter === "changed"
                ? "Nothing is set differently from the modpack."
                : filter === "me"
                  ? "Friends get every file."
                  : "This folder is empty."}
          </p>
        {/if}
      </div>
    </div>
  </section>
{:else if page}
  <!-- The dialog leaves the section out; the tab says why it's empty. -->
  <p class="py-12 text-center text-(--text-muted)">
    No config or other files in this folder yet.
  </p>
{/if}
