<script lang="ts">
  import { plural } from "$lib/format";
  import type { LeftOutFile, LeftOutReason } from "$lib/types";

  // Config files that won't reach friends as they are here, grouped by why. Shown before
  // anything uploads, so nothing is left out without the owner knowing.
  let { files }: { files: LeftOutFile[] } = $props();

  const REASONS: { id: LeftOutReason; label: string }[] = [
    { id: "file_type", label: "Not a file type Atlas shares" },
    { id: "too_big", label: "Over 8 MB" },
    { id: "folder", label: "Not in a folder Atlas shares" },
    { id: "over_limit", label: "Over the limit for one instance" },
  ];

  let open = $state(false);
  // Changed here, but friends still get the modpack's own copy: worth knowing, not a loss.
  const packCopies = $derived(files.filter((f) => f.reason === "pack_copy"));
  const missing = $derived(files.filter((f) => f.reason !== "pack_copy"));
  const groups = $derived(
    [
      ...REASONS.map((r) => ({
        ...r,
        paths: missing.filter((f) => f.reason === r.id).map((f) => f.path),
      })),
      {
        id: "pack_copy",
        label: "Changed here: friends get the modpack's own copy",
        paths: packCopies.map((f) => f.path),
      },
    ].filter((g) => g.paths.length),
  );
</script>

{#if files.length}
  <div class="mt-2.5 border-t border-(--line) pt-2.5">
    <div class="flex items-center gap-3">
      <span
        class="min-w-0 flex-1 text-[12px] {missing.length
          ? 'text-(--warn)'
          : 'text-(--text-muted)'}"
      >
        {#if missing.length}
          {plural(missing.length, "file")} won't be shared
        {:else}
          {plural(packCopies.length, "changed file")} shared as the modpack has
          {packCopies.length === 1 ? "it" : "them"}
        {/if}
      </span>
      <button
        type="button"
        onclick={() => (open = !open)}
        class="text-[12px] text-(--text-muted) underline hover:text-(--text-primary)"
        >{open ? "Hide" : "Show"}</button
      >
    </div>
    {#if open}
      <div class="mt-2 max-h-48 overflow-y-auto">
        {#each groups as group (group.id)}
          <p
            class="mt-2 text-[11.5px] font-medium text-(--text-muted) first:mt-0"
          >
            {group.label}
          </p>
          <ul>
            {#each group.paths as path (path)}
              <li class="truncate font-mono text-[11px] text-(--text-faint)">
                {path}
              </li>
            {/each}
          </ul>
        {/each}
      </div>
    {/if}
  </div>
{/if}
