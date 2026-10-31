<script lang="ts">
  import {
    blocked,
    buildTree,
    canChooseFirstInstall,
    countShared,
    isShared,
    keptAs,
    setFirstInstall,
    setShared,
    type Folder,
  } from "$lib/fileRules";
  import { fileSize, plural } from "$lib/format";
  import type {
    FileRules,
    LeftOutFile,
    LeftOutReason,
    PackInfo,
    ShareFile,
  } from "$lib/types";
  import Button from "../Button.svelte";
  import Icon from "../Icon.svelte";
  import LeftOutFiles from "./LeftOutFiles.svelte";

  // Every file in the instance besides mods and packs, as a tree to tick, like the
  // Modrinth App's export. `rules` stays null until the user changes something, so the
  // dialog only sends new rules when there are some.
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
  const tree = $derived(buildTree(files));
  const counts = $derived(countShared(tree, current));
  const all = $derived(
    counts.get("") ?? { total: 0, shared: 0, sharedBytes: 0 },
  );

  let open = $state<Set<string>>(new Set());
  let query = $state("");
  const matches = $derived(
    query.trim()
      ? files
          .filter((f) => f.path.toLowerCase().includes(query.toLowerCase()))
          .slice(0, 200)
      : [],
  );

  const WHY: Partial<Record<LeftOutReason, string>> = {
    login: "Can hold your Minecraft login, so it's never sent",
    program: "A program, so it's never sent",
    file_type: "Atlas doesn't send this kind of file",
    too_big: "Over 8 MB, too big to send",
  };

  function toggleOpen(path: string) {
    const next = new Set(open);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    open = next;
  }

  const tick = (target: string, on: boolean) =>
    (rules = setShared(current, target, on));
</script>

{#snippet checkbox(
  on: boolean,
  part: boolean,
  label: string,
  onchange: () => void,
)}
  <input
    type="checkbox"
    checked={on}
    indeterminate={part}
    aria-label={label}
    {onchange}
    class="size-4 shrink-0 accent-(--accent)"
  />
{/snippet}

{#snippet file(f: ShareFile, showPath: boolean)}
  {@const on = isShared(current, f.path, f.fromPack)}
  {@const kept = on ? keptAs(current, f.path) : null}
  <div
    class="group flex items-center gap-2 rounded-md px-2 py-1 hover:bg-(--bg-hover)"
  >
    {#if blocked(f)}
      <span
        class="grid size-4 shrink-0 place-items-center text-(--text-faint)"
        title={f.refusal ? WHY[f.refusal] : undefined}
      >
        <Icon name="shield" class="size-3.5" />
      </span>
    {:else}
      {@render checkbox(on, false, f.path, () => tick(f.path, !on))}
    {/if}
    <span
      class="min-w-0 flex-1 truncate font-mono text-[12.5px] {blocked(f)
        ? 'text-(--text-faint)'
        : ''}">{showPath ? f.path : f.path.split("/").pop()}</span
    >
    {#if blocked(f) && f.refusal}
      <small class="shrink-0 text-[12px] text-(--text-faint)"
        >{WHY[f.refusal]}</small
      >
    {:else if kept === "merged"}
      <small class="shrink-0 text-[12px] text-(--accent)"
        >Added to friends' own list</small
      >
    {:else if on && canChooseFirstInstall(f.path)}
      <button
        type="button"
        onclick={() =>
          (rules = setFirstInstall(current, f.path, kept !== "first_install"))}
        class="shrink-0 rounded px-1.5 text-[12px] {kept === 'first_install'
          ? 'text-(--accent)'
          : 'text-(--text-faint) opacity-0 group-hover:opacity-100 focus-visible:opacity-100'}"
        title="Friends get your copy once, then keep their own"
        >{kept === "first_install"
          ? "First install only"
          : "Every update"}</button
      >
    {:else if kept === "first_install"}
      <small class="shrink-0 text-[12px] text-(--accent)"
        >First install only</small
      >
    {:else}
      <small class="shrink-0 text-[12px] text-(--text-faint)"
        >{fileSize(f.sizeBytes)}</small
      >
    {/if}
  </div>
{/snippet}

{#snippet folder(f: Folder, depth: number)}
  {#each f.folders as sub (sub.path)}
    {@const c = counts.get(sub.path)}
    {@const expanded = open.has(sub.path)}
    <div
      class="flex items-center gap-2 rounded-md px-2 py-1 hover:bg-(--bg-hover)"
      style="padding-left: {depth * 18 + 8}px"
    >
      <button
        type="button"
        aria-label={expanded ? `Close ${sub.name}` : `Open ${sub.name}`}
        aria-expanded={expanded}
        onclick={() => toggleOpen(sub.path)}
        class="-ml-1 grid size-5 shrink-0 place-items-center rounded text-(--text-muted) hover:text-(--text-primary)"
      >
        <Icon
          name="chev"
          class="size-3.5 transition-transform {expanded ? 'rotate-90' : ''}"
        />
      </button>
      {#if c && c.total}
        {@render checkbox(
          c.shared === c.total,
          c.shared > 0 && c.shared < c.total,
          sub.path,
          () => tick(sub.path, c.shared < c.total),
        )}
      {:else}
        <span class="size-4 shrink-0"></span>
      {/if}
      <button
        type="button"
        onclick={() => toggleOpen(sub.path)}
        class="min-w-0 flex-1 truncate text-left text-[13px]">{sub.name}</button
      >
      <small class="shrink-0 text-[12px] text-(--text-faint)"
        >{c?.total
          ? `${c.shared} of ${plural(c.total, "file")}`
          : "Can't be sent"}</small
      >
    </div>
    {#if expanded}
      {@render folder(sub, depth + 1)}
    {/if}
  {/each}
  {#each f.files as entry (entry.path)}
    <div style="padding-left: {depth * 18 + 22}px">
      {@render file(entry, false)}
    </div>
  {/each}
{/snippet}

{#if files.length}
  <div class="mt-4 rounded-lg bg-(--bg-raise) px-4 py-3">
    <div class="flex flex-wrap items-baseline gap-x-3">
      <b class="text-[13px] font-medium">Other files</b>
      <small class="text-[12.5px] text-(--text-muted)">
        Sharing {all.shared} of {plural(all.total, "file")} · {fileSize(
          all.sharedBytes,
        )}. Mods and packs are chosen above.
      </small>
    </div>
    {#if pack && pack.files}
      <small class="block text-[12.5px] text-(--text-muted)">
        {plural(pack.files, "file")} come from {pack.name}{pack.versionLabel
          ? ` ${pack.versionLabel}`
          : ""}, so they aren't uploaded.
      </small>
    {/if}

    <div class="mt-2.5 flex items-center gap-2">
      <input
        bind:value={query}
        placeholder="Search {plural(files.length, 'file')}"
        aria-label="Search files"
        class="h-8 min-w-0 flex-1 rounded-md border border-(--line-2) bg-(--bg) px-2.5 text-[13px] outline-none focus:border-(--accent)"
      />
      <Button size="sm" variant="ghost" onclick={() => tick("", true)}
        >Tick all</Button
      >
      <Button size="sm" variant="ghost" onclick={() => tick("", false)}
        >Untick all</Button
      >
    </div>

    <div
      class="mt-1.5 h-72 overflow-y-auto"
      role="tree"
      aria-label="Files to share"
    >
      {#if query.trim()}
        {#each matches as m (m.path)}
          {@render file(m, true)}
        {:else}
          <p class="px-2 py-3 text-[13px] text-(--text-muted)">
            Nothing matches “{query}”.
          </p>
        {/each}
      {:else}
        {@render folder(tree, 0)}
      {/if}
    </div>
    <LeftOutFiles files={leftOut} />
  </div>
{/if}
