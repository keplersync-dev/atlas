<script lang="ts">
  import { api } from "$lib/api";
  import { plural, versionLabel } from "$lib/format";
  import { go } from "$lib/nav.svelte";
  import { resource } from "$lib/resource.svelte";
  import { instances } from "$lib/stores";
  import type { DetectedInstance, LauncherKind } from "$lib/types";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const detected = resource(() => api.listDetectedInstances());
  const launchers = resource(() => api.listLaunchers());

  const LAUNCHER_STYLE: Record<LauncherKind, { mark: string; color: string }> =
    {
      prism: { mark: "P", color: "#8fd16a" },
      multimc: { mark: "M", color: "#8fd16a" },
      modrinth: { mark: "M", color: "#1bd96a" },
      curseforge: { mark: "C", color: "#f16436" },
      official: { mark: "J", color: "#9aa3ad" },
      folder: { mark: "F", color: "#6d7581" },
    };

  // bind:group needs simple values, so each instance gets a string key.
  const keyOf = (d: DetectedInstance) =>
    `${d.launcherKind}:${d.launcherInstanceId}`;

  // Object.groupBy turns one flat list into { prism: [...], modrinth: [...] }.
  const byLauncher = $derived(
    Object.entries(
      Object.groupBy(detected.data ?? [], (d) => d.launcherKind),
    ) as [LauncherKind, DetectedInstance[]][],
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
</script>

<Dialog
  title="Track instances"
  description="Found in the launchers on this PC. Atlas only reads them until you share or update."
  wide
  {onclose}
  onsubmit={submit}
>
  {#if detected.loading}
    <p class="py-4 text-[12.5px] text-(--text-muted)">
      Looking through your launchers…
    </p>
  {:else if detected.error}
    <p class="py-4 text-[12.5px] text-(--danger)">{detected.error}</p>
  {:else if byLauncher.length === 0}
    <p class="py-4 text-[12.5px] text-(--text-muted)">
      Atlas didn't find Prism, the Modrinth App, CurseForge or the Minecraft
      Launcher. Choose a folder instead.
    </p>
  {:else}
    {#if untracked.length > 1}
      <div class="mb-1 flex justify-end">
        <button
          type="button"
          class="text-xs text-(--accent) hover:underline"
          onclick={toggleAll}
        >
          {allNewSelected
            ? "Select none"
            : `Select all ${untracked.length} new`}
        </button>
      </div>
    {/if}

    {#each byLauncher as [kind, list] (kind)}
      <section class="mb-2">
        <div
          class="flex items-center gap-2.5 px-2 pt-2 pb-1 text-[12.5px] font-semibold"
        >
          <span
            class="grid size-5 place-items-center rounded-[5px] text-[10px] font-bold text-[#0b0d10]"
            style="background: {LAUNCHER_STYLE[kind].color}"
            >{LAUNCHER_STYLE[kind].mark}</span
          >
          {launcherName(kind)}
          <small class="font-normal text-(--text-faint)"
            >{plural(list.length, "instance")}</small
          >
        </div>
        {#each list as d (keyOf(d))}
          {@const tracked = !!d.instanceId}
          <label
            class="flex items-center gap-3 rounded-md py-1.5 pr-2 pl-9.5 {tracked
              ? 'opacity-50'
              : 'cursor-pointer hover:bg-(--bg-hover)'}"
          >
            <input
              type="checkbox"
              bind:group={selected}
              value={keyOf(d)}
              disabled={tracked}
              class="size-3.75 accent-(--accent)"
            />
            <span class="min-w-0 flex-1">
              <b class="block truncate font-medium">{d.name}</b>
              <small class="block text-[11.5px] text-(--text-faint)">
                {tracked
                  ? "Already tracked"
                  : versionLabel(d.loader, d.minecraftVersion)}
              </small>
            </span>
          </label>
        {/each}
      </section>
    {/each}
  {/if}

  <!-- Expandable section: plain $state toggles it, {#if} shows it. -->
  <div class="mt-2 border-t border-(--line) pt-3">
    {#if !showFolder}
      <button
        type="button"
        class="text-xs text-(--accent) hover:underline"
        onclick={() => (showFolder = true)}
      >
        Not listed? Choose a folder
      </button>
    {:else}
      <h3 class="mb-1.5 text-xs font-semibold text-(--text-muted)">
        Game folder
      </h3>
      <p class="mb-2 text-xs text-(--text-faint)">
        The folder that contains mods/ and config/.
      </p>
      <div class="flex gap-2">
        <input
          bind:value={folder}
          placeholder="D:\Minecraft\My Instance"
          class="h-8.5 min-w-0 flex-1 rounded-md border border-(--line-2) bg-(--bg) px-3 font-mono text-xs outline-none focus:border-(--accent)"
        />
        <Button disabled={!folder.trim() || submitting} onclick={trackFolder}
          >Track folder</Button
        >
      </div>
    {/if}
  </div>

  {#snippet footer()}
    {#if error}<span class="mr-auto text-xs text-(--danger)" role="alert"
        >{error}</span
      >{/if}
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
