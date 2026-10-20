<script lang="ts">
  import type { SharedInstance } from "$lib/types";
  import { plural } from "$lib/format";
  import { attentionOf } from "./SharedRow.svelte";

  // "2 new · 1 update · 1 to publish" pills for a list of shared instances.
  let { items }: { items: SharedInstance[] } = $props();

  const need = $derived(attentionOf(items));
</script>

{#if need.new}<span
    class="rounded-full bg-(--accent)/15 px-2 py-0.5 text-[11px] font-semibold text-(--accent)"
    >{need.new} new</span
  >{/if}
{#if need.update}<span
    class="rounded-full bg-(--warn)/15 px-2 py-0.5 text-[11px] font-semibold text-(--warn)"
    >{plural(need.update, "update")}</span
  >{/if}
{#if need.changes}<span
    class="rounded-full bg-(--accent)/15 px-2 py-0.5 text-[11px] font-semibold text-(--accent)"
    >{plural(need.changes, "to publish", "to publish")}</span
  >{/if}
