<script lang="ts">
  import { api } from "$lib/api";
  import { confirmAction } from "$lib/confirm.svelte";
  import { fileSize, plural, timeAgo } from "$lib/format";
  import { fuzzyFilter } from "$lib/fuzzy";
  import { resource } from "$lib/resource.svelte";
  import { social } from "$lib/stores";
  import { showDone, showError } from "$lib/toasts.svelte";
  import type { MyUpload } from "$lib/types";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";
  import Icon from "../Icon.svelte";

  // What fills your upload space: every file you uploaded, what still uses it, and a way
  // to free the space a file nothing uses takes now (the server also does it after a day).
  let { onclose }: { onclose: () => void } = $props();

  const uploads = resource(() => api.listMyUploads());
  const storage = $derived(social.session?.user?.storage ?? null);
  const pct = $derived(
    storage?.quotaBytes
      ? Math.min(100, (storage.usedBytes / storage.quotaBytes) * 100)
      : 0,
  );

  let query = $state("");
  type Sort = "name" | "size";
  let sortBy = $state<Sort>("size");
  let asc = $state(false);
  function sortOn(s: Sort) {
    if (sortBy === s) asc = !asc;
    else {
      sortBy = s;
      // Names A to Z, sizes biggest first: what you'd look for first.
      asc = s === "name";
    }
  }

  const rows = $derived.by(() => {
    const list = uploads.data ?? [];
    if (query.trim()) return fuzzyFilter(list, query, (f) => [f.fileName]);
    const dir = asc ? 1 : -1;
    return list.toSorted(
      (a, b) =>
        (sortBy === "size"
          ? a.sizeBytes - b.sizeBytes
          : baseName(a.fileName).localeCompare(baseName(b.fileName))) * dir,
    );
  });

  let removing = $state<string | null>(null);
  async function remove(f: MyUpload) {
    const ok = await confirmAction({
      title: `Remove ${baseName(f.fileName)}?`,
      message: `No pack uses it, so nobody loses anything. It frees ${fileSize(f.sizeBytes)}.`,
      confirmLabel: "Remove",
      danger: true,
    });
    if (!ok) return;
    removing = f.sha256;
    try {
      await api.removeMyUpload(f.sha256);
      uploads.reload();
      await social.load();
      showDone(`Removed ${baseName(f.fileName)}.`);
    } catch (e) {
      showError((e as Error).message);
    } finally {
      removing = null;
    }
  }

  // Name (with its folder and date under it), size, what uses it, and Remove on its own
  // at the right edge so the buttons line up.
  const COLS =
    "grid grid-cols-[minmax(0,1fr)_64px_minmax(140px,0.8fr)_80px] items-center gap-3 px-3";
  // Sorted and shown by the file's own name; its folder sits under it.
  const baseName = (path: string) => path.split("/").pop() ?? path;
  const folderOf = (path: string) =>
    path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "";
</script>

{#snippet sortHeader(s: Sort, label: string)}
  <button
    type="button"
    onclick={() => sortOn(s)}
    class="flex items-center gap-1 text-left font-medium hover:text-(--text-primary)"
  >
    {label}
    {#if sortBy === s && !query.trim()}
      <Icon name="chev" class="size-3 {asc ? '-rotate-90' : 'rotate-90'}" />
    {/if}
  </button>
{/snippet}

<Dialog
  title="Your uploads"
  description="Files that aren't on Modrinth or CurseForge, stored on Atlas for friends to download."
  wide
  {onclose}
>
  {#if storage}
    <div class="h-1.5 overflow-hidden rounded-full bg-(--line)">
      <div
        class="h-full rounded-full {pct >= 80
          ? 'bg-(--warn)'
          : 'bg-(--accent)'}"
        style="width: {pct}%"
      ></div>
    </div>
    <p class="mt-1.5 text-[13px] text-(--text-muted) tabular-nums">
      {fileSize(storage.usedBytes)} of {fileSize(storage.quotaBytes)}
      {#if uploads.data}· {plural(uploads.data.length, "file")}{/if}
    </p>
  {/if}

  {#if uploads.loading && !uploads.data}
    <p class="py-6 text-[13px] text-(--text-muted)">Loading your uploads…</p>
  {:else if uploads.error}
    <p class="py-6 text-[13px] text-(--danger)">
      Couldn't load your uploads: {uploads.error}
      <button class="ml-1 underline" onclick={uploads.reload}>Try again</button>
    </p>
  {:else if uploads.data && !uploads.data.length}
    <p class="py-8 text-center text-[13px] text-(--text-muted)">
      Nothing uploaded. Mods from Modrinth and CurseForge never need it.
    </p>
  {:else if uploads.data}
    <label class="relative mt-3 block">
      <span
        class="pointer-events-none absolute top-1/2 left-2.5 flex -translate-y-1/2 text-(--text-faint)"
      >
        <Icon name="search" class="size-3.5" />
      </span>
      <input
        bind:value={query}
        placeholder="Search uploads"
        aria-label="Search uploads"
        class="h-8 w-full rounded-md border border-(--line-2) bg-(--bg) pr-2.5 pl-8 text-[13px] outline-none placeholder:text-(--text-faint) focus:border-(--accent)"
      />
    </label>

    <div
      role="table"
      aria-label="Your uploads"
      class="mt-3 overflow-hidden rounded-lg border border-(--line)"
    >
      <div
        role="row"
        class="{COLS} border-b border-(--line) bg-(--bg) py-2 text-[13px] text-(--text-muted)"
      >
        {@render sortHeader("name", "Name")}
        {@render sortHeader("size", "Size")}
        <span>Used in</span>
        <span></span>
      </div>
      <div class="max-h-80 overflow-y-auto">
        {#each rows as f (f.sha256)}
          <div
            role="row"
            class="{COLS} border-b border-(--line) py-2 text-[13px] last:border-b-0 hover:bg-(--bg-hover)"
          >
            <span class="min-w-0" title={f.fileName}>
              <span class="block truncate text-[14px]"
                >{baseName(f.fileName)}</span
              >
              <small class="block truncate text-[13px] text-(--text-faint)"
                >{folderOf(f.fileName)
                  ? `${folderOf(f.fileName)} · `
                  : ""}uploaded
                {timeAgo(f.uploadedAt)}</small
              >
            </span>
            <span class="text-(--text-muted) tabular-nums"
              >{fileSize(f.sizeBytes)}</span
            >
            <span class="min-w-0">
              {#if f.blocked}
                <span
                  class="rounded-full bg-(--danger)/12 px-2 py-0.5 text-(--danger)"
                  title="Taken down after a report. It stays listed so it can't be uploaded again."
                  >Taken down</span
                >
              {:else if f.usedIn.length}
                <span
                  class="block truncate text-(--text-muted)"
                  title={f.usedIn.join(", ")}>{f.usedIn.join(", ")}</span
                >
              {:else}
                <span
                  class="rounded-full bg-(--warn)/12 px-2 py-0.5 whitespace-nowrap text-(--warn)"
                  >Not used anymore</span
                >
              {/if}
            </span>
            <!-- Its own column, so every row's button lines up at the right edge. -->
            <span class="flex justify-end">
              {#if !f.blocked && !f.usedIn.length}
                <Button
                  size="sm"
                  disabled={removing !== null}
                  onclick={() => remove(f)}
                  >{removing === f.sha256 ? "Removing…" : "Remove"}</Button
                >
              {/if}
            </span>
          </div>
        {:else}
          <p class="px-3 py-4 text-[13px] text-(--text-muted)">
            No uploads match “{query.trim()}”.
          </p>
        {/each}
      </div>
    </div>
    <p class="mt-2 text-[13px] text-(--text-faint)">
      Files no pack uses are also removed by themselves a day later.
    </p>
  {/if}

  {#snippet footer()}
    <Button onclick={onclose}>Done</Button>
  {/snippet}
</Dialog>
