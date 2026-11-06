<script lang="ts">
  import { api } from "$lib/api";
  import { sameRules } from "$lib/publishChanges";
  import { showError } from "$lib/toasts.svelte";
  import type { FileRules, Id, PublishPlan } from "$lib/types";
  import Button from "../Button.svelte";
  import FileTree from "../dialogs/FileTree.svelte";

  // Which config and other files friends get. A setting of the pack rather than of one
  // update, so it lives here; ticks are saved as they change and go out with the next
  // update. Loading reads the whole folder (and the modpack, if there is one), so only
  // when asked.
  let { instanceId }: { instanceId: Id } = $props();

  let plan = $state<PublishPlan | null>(null);
  let loading = $state(false);
  let rules = $state<FileRules | null>(null);
  let saved = $state<FileRules | null>(null);
  let note = $state<string | null>(null);

  async function load() {
    loading = true;
    try {
      plan = await api.preparePublish(instanceId);
      saved = plan.rules;
    } catch (e) {
      showError((e as Error).message);
    } finally {
      loading = false;
    }
  }

  // Saved a moment after the last tick, so ticking a whole folder is one save.
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const next = rules;
    if (!next || !plan || !saved || sameRules(next, saved, plan.files)) return;
    clearTimeout(timer);
    timer = setTimeout(async () => {
      try {
        await api.setFileRules(instanceId, next);
        saved = next;
        note = "Saved. Friends get it with your next update.";
      } catch (e) {
        showError((e as Error).message);
      }
    }, 600);
    return () => clearTimeout(timer);
  });
</script>

<section class="mt-6">
  {#if !plan}
    <div class="flex items-center gap-3 rounded-lg bg-(--bg-raise) px-3.5 py-3">
      <div class="min-w-0 flex-1">
        <b class="block font-medium">Configs and other files</b>
        <small class="block text-[13px] text-(--text-muted)">
          Atlas shares config and script folders for you. Choose what else
          friends get, or what they don't.
        </small>
      </div>
      <Button onclick={load} disabled={loading}
        >{loading ? "Reading the folder…" : "Choose files"}</Button
      >
    </div>
  {:else}
    {#if note}<p class="mb-1 text-[13px] text-(--text-muted)" role="status">
        {note}
      </p>{/if}
    <FileTree
      files={plan.files}
      saved={saved ?? plan.rules}
      leftOut={plan.leftOut}
      bind:rules
    />
  {/if}
</section>
