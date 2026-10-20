<script lang="ts">
  import { fileSize } from "$lib/format";
  import { social } from "$lib/stores";

  // How much of the upload space your custom files use.

  const storage = $derived(social.session?.user?.storage ?? null);
  const pct = $derived(
    storage ? Math.min(100, (storage.usedBytes / storage.quotaBytes) * 100) : 0,
  );
  // Amber from 80%, so it's noticed before a share gets refused.
  const fill = $derived(pct >= 80 ? "bg-(--warn)" : "bg-(--accent)");
</script>

{#if storage}
  <div>
    <div class="flex items-baseline justify-between">
      <b class="font-semibold">Upload space</b>
      <span class="text-xs text-(--text-muted) tabular-nums"
        >{fileSize(storage.usedBytes)} of {fileSize(storage.quotaBytes)}</span
      >
    </div>
    <div class="mt-2 h-2 overflow-hidden rounded-full bg-(--bg-raise-2)">
      <div class="h-full rounded-full {fill}" style="width: {pct}%"></div>
    </div>
    <p class="mt-2 text-xs text-(--text-faint)">
      Custom files you share are stored on Atlas, up to {fileSize(
        storage.maxFileBytes,
      )} each. Mods from Modrinth and CurseForge don't count.
    </p>
  </div>
{/if}
