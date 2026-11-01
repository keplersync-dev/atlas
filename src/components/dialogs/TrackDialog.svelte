<script lang="ts">
  import { fuzzy } from "$lib/fuzzy";
  import { api, type ApiError } from "$lib/api";
  import { plural, versionLabel } from "$lib/format";
  import { go } from "$lib/nav.svelte";
  import { resource } from "$lib/resource.svelte";
  import { instances } from "$lib/stores";
  import type { DetectedInstance, LauncherKind } from "$lib/types";
  import Button from "../Button.svelte";
  import ContentIcon from "../ContentIcon.svelte";
  import Dialog from "../Dialog.svelte";
  import Icon from "../Icon.svelte";
  import LauncherIcon from "../LauncherIcon.svelte";

  let { onclose }: { onclose: () => void } = $props();

  // Look for launchers again each time: one installed while Atlas runs shows up.
  const looked = api.detectLaunchers().catch(() => null);
  const detected = resource(() =>
    looked.then(() => api.listDetectedInstances()),
  );
  const launchers = resource(() => looked.then(() => api.listLaunchers()));

  // bind:group needs simple values, so each instance gets a string key.
  const keyOf = (d: DetectedInstance) =>
    `${d.launcherKind}:${d.launcherInstanceId}`;

  let query = $state("");
  const matches = (d: DetectedInstance) => fuzzy(query, d.name) !== null;
  // Grouped by launcher; new ones first, ones already added at the end of each group.
  const byLauncher = $derived(
    (
      Object.entries(
        Object.groupBy(
          (detected.data ?? []).filter(matches),
          (d) => d.launcherKind,
        ),
      ) as [LauncherKind, DetectedInstance[]][]
    ).map(
      ([kind, list]) =>
        [
          kind,
          list.toSorted(
            (a, b) =>
              Number(!!a.instanceId) - Number(!!b.instanceId) ||
              a.name.localeCompare(b.name),
          ),
        ] as const,
    ),
  );
  const untracked = $derived(detected.data?.filter((d) => !d.instanceId) ?? []);
  const launcherName = (kind: LauncherKind) =>
    launchers.data?.find((l) => l.kind === kind)?.name ?? kind;

  let selected = $state<string[]>([]);
  let showFolder = $state(false);
  let folder = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  const allNewSelected = $derived(
    untracked.length > 0 && selected.length === untracked.length,
  );

  function toggleAll() {
    selected = allNewSelected ? [] : untracked.map(keyOf);
  }

  /** Runs the save, refreshes the sidebar, and opens the instance if there's exactly one. */
  async function finish(save: () => Promise<{ id: string }[]>) {
    submitting = true;
    error = null;
    try {
      const added = await save();
      await instances.load();
      onclose();
      if (added.length === 1) go({ page: "instance", id: added[0].id });
    } catch (err) {
      error = (err as Error).message;
    } finally {
      submitting = false;
    }
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    // Turn the string keys back into the refs the API wants.
    const byKey = new Map(untracked.map((d) => [keyOf(d), d]));
    const refs = selected
      .map((k) => byKey.get(k))
      .filter((d) => d !== undefined)
      .map((d) => ({
        launcherKind: d.launcherKind,
        launcherInstanceId: d.launcherInstanceId,
      }));
    finish(() => api.trackInstances(refs));
  }

  function trackFolder() {
    // In the real app this path comes from a native folder picker (Tauri's dialog plugin).
    finish(async () => [await api.trackFolder(folder.trim())]);
  }

  // A launcher Atlas didn't find on its own, like a portable Prism: its instances then
  // show up above, with everything Atlas can do in that launcher.
  let launcherFolder = $state("");
  let addingLauncher = $state(false);
  const ADDABLE = [
    "prism",
    "multimc",
    "modrinth",
    "curseforge",
    "official",
  ] as const;

  // A Modrinth pack, installed by a launcher that takes them; its new instance then shows
  // up above (after "Look again").
  const importers = $derived(
    launchers.data?.filter(
      (l) => l.kind === "prism" || l.kind === "modrinth",
    ) ?? [],
  );
  let imported = $state<string | null>(null);

  async function importPack(launcher: { id: string; name: string }) {
    error = null;
    try {
      if (await api.importMrpack(launcher.id)) imported = launcher.name;
    } catch (err) {
      error = (err as Error).message;
    }
  }

  async function addLauncher() {
    addingLauncher = true;
    error = null;
    try {
      for (const kind of ADDABLE) {
        try {
          await api.addLauncher(kind, launcherFolder.trim());
          launcherFolder = "";
          launchers.reload();
          detected.reload();
          return;
        } catch (err) {
          // Not this kind of launcher: try the next. Anything else (already added) stops.
          if ((err as ApiError).code !== "invalid_input") throw err;
        }
      }
      error =
        "That folder isn't a Prism, MultiMC, Modrinth App, CurseForge or Minecraft Launcher install.";
    } catch (err) {
      error = (err as Error).message;
    } finally {
      addingLauncher = false;
    }
  }
</script>

<Dialog
  title="Track instances"
  description="Found in the launchers on this PC. Atlas only reads them until you share or update."
  wide
  {onclose}
  onsubmit={submit}
>
  {#if detected.loading}
    <p class="py-4 text-[13px] text-(--text-muted)">
      Looking through your launchers…
    </p>
  {:else if detected.error}
    <p class="py-4 text-[13px] text-(--danger)">{detected.error}</p>
  {:else if byLauncher.length === 0}
    <p class="py-4 text-[13px] text-(--text-muted)">
      Atlas didn't find Prism, the Modrinth App, CurseForge or the Minecraft
      Launcher. Choose a folder instead.
    </p>
  {:else}
    <div class="mb-3 flex items-center gap-3">
      <label class="relative block min-w-0 flex-1">
        <span
          class="pointer-events-none absolute top-1/2 left-2.5 flex -translate-y-1/2 text-(--text-faint)"
        >
          <Icon name="search" class="size-3.5" />
        </span>
        <input
          bind:value={query}
          placeholder="Search {plural(detected.data?.length ?? 0, 'instance')}"
          aria-label="Search instances"
          class="h-8 w-full rounded-md border border-(--line-2) bg-(--bg) pr-2.5 pl-8 text-[13px] outline-none placeholder:text-(--text-faint) focus:border-(--accent)"
        />
      </label>
      {#if untracked.length > 1}
        <button
          type="button"
          class="shrink-0 rounded px-1.5 py-1 text-[13px] text-(--accent) hover:bg-(--bg-hover)"
          onclick={toggleAll}
        >
          {allNewSelected ? "Select none" : `Select all ${untracked.length}`}
        </button>
      {/if}
    </div>

    {#each byLauncher as [kind, list] (kind)}
      <section class="mb-4">
        <h3 class="mb-1.5 flex items-center gap-2.5 text-[14px] font-semibold">
          <LauncherIcon {kind} class="size-6" />
          {launcherName(kind)}
          <span class="font-normal text-(--text-faint)">{list.length}</span>
        </h3>
        <div class="grid gap-1">
          {#each list as d (keyOf(d))}
            {@const tracked = !!d.instanceId}
            {@const on = selected.includes(keyOf(d))}
            <label
              class="flex items-center gap-3 rounded-lg border px-3 py-2 transition-colors
                {tracked
                ? 'border-transparent opacity-60'
                : on
                  ? 'cursor-pointer border-(--accent)/60 bg-(--accent)/8'
                  : 'cursor-pointer border-(--line) hover:bg-(--bg-hover)'}"
            >
              <input
                type="checkbox"
                bind:group={selected}
                value={keyOf(d)}
                disabled={tracked}
                class="size-4 shrink-0 accent-(--accent) {tracked
                  ? 'invisible'
                  : ''}"
              />
              <ContentIcon
                src={d.icon}
                seed={d.gameDir}
                class="size-9 rounded-md"
              />
              <span class="min-w-0 flex-1">
                <b class="block truncate font-medium">{d.name}</b>
                <small class="block truncate text-[13px] text-(--text-faint)">
                  {versionLabel(d.loader, d.minecraftVersion)}
                </small>
              </span>
              {#if tracked}
                <span
                  class="flex shrink-0 items-center gap-1 text-[13px] text-(--text-muted)"
                  ><Icon name="check" class="size-3.5" />Already added</span
                >
              {/if}
            </label>
          {/each}
        </div>
      </section>
    {:else}
      <p class="py-4 text-[13px] text-(--text-muted)">
        No instances match “{query.trim()}”.
      </p>
    {/each}
  {/if}

  <!-- Expandable section: plain $state toggles it, {#if} shows it. -->
  <div class="mt-2 border-t border-(--line) pt-3">
    {#if !showFolder}
      <button
        type="button"
        class="text-[13px] text-(--accent) hover:underline"
        onclick={() => (showFolder = true)}
      >
        Not listed? Choose a folder, a pack or a launcher
      </button>
    {:else}
      <h3 class="mb-1.5 text-[13px] font-semibold text-(--text-muted)">
        Game folder
      </h3>
      <p class="mb-2 text-[13px] text-(--text-faint)">
        The folder that contains mods/ and config/.
      </p>
      <div class="flex gap-2">
        <input
          bind:value={folder}
          placeholder="D:\Minecraft\My Instance"
          class="h-8.5 min-w-0 flex-1 rounded-md border border-(--line-2) bg-(--bg) px-3 font-mono text-[13px] outline-none focus:border-(--accent)"
        />
        <Button disabled={!folder.trim() || submitting} onclick={trackFolder}
          >Track folder</Button
        >
      </div>

      {#if importers.length}
        <h3 class="mt-4 mb-1.5 text-[13px] font-semibold text-(--text-muted)">
          Modrinth pack (.mrpack)
        </h3>
        <p class="mb-2 text-[13px] text-(--text-faint)">
          {imported
            ? `Finish adding it in ${imported}, then look again here to track it.`
            : "A launcher installs it as a new instance, which you can then track."}
        </p>
        <div class="flex flex-wrap gap-2">
          {#each importers as l (l.id)}
            <Button onclick={() => importPack(l)}>Add to {l.name}</Button>
          {/each}
          {#if imported}
            <Button variant="ghost" onclick={() => detected.reload()}
              >Look again</Button
            >
          {/if}
        </div>
      {/if}

      <h3 class="mt-4 mb-1.5 text-[13px] font-semibold text-(--text-muted)">
        Launcher folder
      </h3>
      <p class="mb-2 text-[13px] text-(--text-faint)">
        For a launcher Atlas didn't find, like a portable Prism: the folder it
        runs from. Its instances then appear above.
      </p>
      <div class="flex gap-2">
        <input
          bind:value={launcherFolder}
          placeholder="D:\PrismLauncher"
          class="h-8.5 min-w-0 flex-1 rounded-md border border-(--line-2) bg-(--bg) px-3 font-mono text-[13px] outline-none focus:border-(--accent)"
        />
        <Button
          disabled={!launcherFolder.trim() || addingLauncher}
          onclick={addLauncher}
          >{addingLauncher ? "Adding…" : "Add launcher"}</Button
        >
      </div>
    {/if}
  </div>

  {#snippet footer()}
    <!-- Its own line, so a long message never pushes the buttons out of place. -->
    {#if error}<p class="basis-full text-[13px] text-(--danger)" role="alert">
        {error}
      </p>{/if}
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button
      type="submit"
      variant="primary"
      disabled={submitting || selected.length === 0}
    >
      {submitting
        ? "Tracking…"
        : selected.length
          ? `Track ${plural(selected.length, "instance")}`
          : "Track selected"}
    </Button>
  {/snippet}
</Dialog>
