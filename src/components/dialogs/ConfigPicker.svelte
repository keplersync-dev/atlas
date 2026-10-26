<script lang="ts">
  import { fileSize, plural } from "$lib/format";
  import type { ConfigGroup } from "$lib/types";
  import Button from "../Button.svelte";

  // Which config folders (one per mod, usually) and loose config files go out with the
  // instance. `excluded` stays null until the user changes something here, so the dialog
  // only sends a new list when there is one.
  let {
    configs,
    excluded = $bindable(null),
  }: {
    configs: ConfigGroup[];
    excluded: string[] | null;
  } = $props();

  let open = $state(false);
  let query = $state("");

  const off = $derived(
    excluded ?? configs.filter((g) => !g.shared).map((g) => g.key),
  );
  const shared = $derived(configs.filter((g) => !off.includes(g.key)));
  const totalFiles = $derived(configs.reduce((n, g) => n + g.files, 0));
  const sharedFiles = $derived(shared.reduce((n, g) => n + g.files, 0));
  const sharedBytes = $derived(shared.reduce((n, g) => n + g.sizeBytes, 0));
  const shown = $derived(
    configs.filter((g) => g.key.toLowerCase().includes(query.toLowerCase())),
  );

  const label = (key: string) => key.replace(/^config\//, "");

  function toggle(key: string, on: boolean) {
    excluded = on ? off.filter((k) => k !== key) : [...off, key];
  }
</script>

{#if configs.length}
  <div class="mt-4 rounded-lg bg-(--bg-raise) px-4 py-3">
    <div class="flex items-center gap-3">
      <div class="min-w-0 flex-1">
        <b class="block text-[12.5px] font-medium">Config files</b>
        <small class="block text-[11.5px] text-(--text-faint)">
          Sharing {sharedFiles} of {plural(totalFiles, "file")} · {fileSize(
            sharedBytes,
          )}. Some mods keep personal settings here.
        </small>
      </div>
      <Button size="sm" variant="ghost" onclick={() => (open = !open)}
        >{open ? "Done" : "Choose"}</Button
      >
    </div>
    {#if open}
      <input
        bind:value={query}
        placeholder="Filter {configs.length} folders and files"
        class="mt-2.5 h-7.5 w-full rounded-md border border-(--line-2) bg-(--bg) px-2.5 text-[12.5px] outline-none focus:border-(--accent)"
      />
      <div class="mt-1.5 max-h-64 overflow-y-auto">
        {#each shown as group (group.key)}
          <label
            class="flex cursor-pointer items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-(--bg-hover)"
          >
            <input
              type="checkbox"
              checked={!off.includes(group.key)}
              onchange={(e) => toggle(group.key, e.currentTarget.checked)}
              class="accent-(--accent)"
            />
            <span class="min-w-0 flex-1 truncate font-mono text-xs"
              >{label(group.key)}</span
            >
            <small class="shrink-0 text-[11px] text-(--text-faint)"
              >{group.key.endsWith("/")
                ? plural(group.files, "file")
                : fileSize(group.sizeBytes)}</small
            >
          </label>
        {:else}
          <p class="px-2 py-3 text-[12.5px] text-(--text-muted)">
            Nothing matches “{query}”.
          </p>
        {/each}
      </div>
    {/if}
  </div>
{/if}
