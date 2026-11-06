<script lang="ts">
  import { workLabel } from "$lib/format";
  import type { InstanceWork, PublishItem } from "$lib/types";
  import { names } from "./SharePicker.svelte";

  // The share and publish dialogs' footer note, next to the buttons so it's always in
  // view: the upload-rights confirmation when files get uploaded, then progress, or why
  // sharing failed, or `hint`.
  let {
    checked = $bindable(false),
    uploads,
    submitting,
    work,
    error,
    hint = null,
  }: {
    checked: boolean;
    uploads: PublishItem[];
    submitting: boolean;
    work: InstanceWork | undefined;
    error: string | null;
    hint?: string | null;
  } = $props();

  const progress = $derived(submitting && work ? workLabel(work) : null);
</script>

{#if uploads.length}
  <div class="mr-auto flex min-w-0 flex-col gap-1">
    <label class="tap flex cursor-pointer items-center gap-2 text-[13px]">
      <input type="checkbox" bind:checked class="accent-(--accent)" />
      <span
        >I made <b class="font-medium">{names(uploads)}</b> or have permission to
        share {uploads.length === 1 ? "it" : "them"}</span
      >
    </label>
    {#if progress ?? error}
      <span
        class="text-[13px] tabular-nums {progress
          ? 'text-(--text-muted)'
          : 'text-(--danger)'}"
        role={progress ? undefined : "alert"}>{progress ?? error}</span
      >
    {/if}
  </div>
{:else if progress}
  <span class="mr-auto text-[13px] text-(--text-muted) tabular-nums"
    >{progress}</span
  >
{:else if error}
  <span class="mr-auto text-[13px] text-(--danger)" role="alert">{error}</span>
{:else if hint}
  <span class="mr-auto text-[13px] text-(--text-muted)">{hint}</span>
{/if}
