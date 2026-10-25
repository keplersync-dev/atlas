<script lang="ts">
  import { api } from "$lib/api";
  import { updates } from "$lib/stores";
  import Button from "./Button.svelte";

  const label = $derived(
    updates.installing
      ? updates.progress === null
        ? "Downloading…"
        : `Downloading ${Math.round(updates.progress * 100)}%`
      : "Restart to update",
  );

  function download() {
    api.openReleasePage().catch((e) => (updates.error = (e as Error).message));
  }
</script>

<!-- Above the account: a newer Atlas is ready, or the server needs one to keep syncing. -->
{#if updates.available || updates.required}
  <div
    class="mx-2.5 mb-2 rounded-lg border px-3 py-2.5 {updates.required
      ? 'border-(--warn)/50 bg-(--warn)/8'
      : 'border-(--line-2) bg-(--bg-raise-2)'}"
    role={updates.required ? "alert" : "status"}
  >
    <b class="block text-[12.5px] font-medium">
      {updates.required
        ? "Update Atlas to keep syncing"
        : `Atlas ${updates.available?.version} is ready`}
    </b>
    <small class="mt-0.5 block text-[11.5px] leading-snug text-(--text-faint)">
      {updates.required
        ? "This version is too old for the Atlas server. Your instances still work on this PC."
        : "Atlas closes for a few seconds while it updates."}
    </small>
    {#if updates.available}
      <Button
        size="sm"
        variant={updates.required ? "warn" : "primary"}
        class="mt-2 w-full"
        disabled={updates.installing}
        onclick={() => updates.install()}>{label}</Button
      >
    {:else}
      <Button size="sm" variant="warn" class="mt-2 w-full" onclick={download}
        >Download the new version</Button
      >
    {/if}
    {#if updates.error}
      <p class="mt-1.5 text-[11.5px] text-(--danger)" role="alert">
        {updates.error}
      </p>
    {/if}
  </div>
{/if}
