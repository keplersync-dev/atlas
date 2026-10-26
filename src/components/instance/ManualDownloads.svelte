<script lang="ts">
  import { api } from "$lib/api";
  import type { InstanceSummary } from "$lib/types";
  import Button from "../Button.svelte";

  // An update waiting on files only CurseForge's website hands out: two short steps, and
  // each file's download again in case the browser tab was closed.
  let { instance }: { instance: InstanceSummary } = $props();

  const files = $derived(instance.progress?.manualDownloads ?? []);
  const one = $derived(files.length === 1);
  let error = $state<string | null>(null);

  function open(pageUrl: string) {
    error = null;
    api
      .openManualDownload(instance.id, pageUrl)
      .catch((e) => (error = (e as Error).message));
  }
</script>

{#snippet step(n: number)}
  <span
    class="grid size-5 shrink-0 place-items-center rounded-full bg-(--accent)/15 text-[11px] font-semibold text-(--accent)"
    >{n}</span
  >
{/snippet}

<div class="text-[12.5px]">
  <ol class="grid gap-1.5">
    <li class="flex items-center gap-2.5">
      {@render step(1)}
      <span
        >Your browser downloads {one ? "it" : "them"} in a few seconds</span
      >
    </li>
    <li class="flex items-center gap-2.5">
      {@render step(2)}
      <span>Leave {one ? "it" : "them"} in Downloads. Atlas does the rest</span>
    </li>
  </ol>
  <ul class="mt-3 grid max-h-36 gap-1.5 overflow-y-auto">
    {#each files as f (f.pageUrl)}
      <li class="flex items-center gap-3">
        <span class="min-w-0 flex-1">
          <b class="block truncate font-medium">{f.contentName}</b>
          <small class="block truncate text-[11.5px] text-(--text-faint)"
            >{f.fileName}</small
          >
        </span>
        <Button size="sm" onclick={() => open(f.pageUrl)}>Download</Button>
      </li>
    {/each}
  </ul>
  {#if error}
    <p class="mt-2 text-(--danger)" role="alert">{error}</p>
  {/if}
</div>
