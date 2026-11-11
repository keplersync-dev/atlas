<script lang="ts">
  import { api } from "$lib/api";
  import { openDialog } from "$lib/dialogs.svelte";
  import { sameRules } from "$lib/publishChanges";
  import { showError } from "$lib/toasts.svelte";
  import type { FileRules, Id, PublishPlan } from "$lib/types";
  import Button from "../Button.svelte";
  import FileTree from "../dialogs/FileTree.svelte";

  // Which config and other files friends get. A setting of the pack rather than of one
  // update, so it has its own tab; changes are saved as they're made and go out with the
  // next update. Reading the folder (and the modpack, if there is one) starts when the
  // tab opens.
  let { instanceId }: { instanceId: Id } = $props();

  let plan = $state<PublishPlan | null>(null);
  let failed = $state<string | null>(null);
  let rules = $state<FileRules | null>(null);
  let saved = $state<FileRules | null>(null);

  async function load() {
    failed = null;
    try {
      plan = await api.preparePublish(instanceId);
      saved = plan.rules;
    } catch (e) {
      failed = (e as Error).message;
    }
  }
  load();

  // Saved a moment after the last change, so setting a whole folder is one save.
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const next = rules;
    if (!next || !plan || !saved || sameRules(next, saved, plan.files)) return;
    clearTimeout(timer);
    timer = setTimeout(async () => {
      try {
        await api.setFileRules(instanceId, next);
        saved = next;
      } catch (e) {
        showError((e as Error).message);
      }
    }, 600);
    return () => clearTimeout(timer);
  });

  // The last change may still be waiting to be saved; the dialog reads the saved rules.
  async function send() {
    if (rules && plan && saved && !sameRules(rules, saved, plan.files)) {
      clearTimeout(timer);
      try {
        await api.setFileRules(instanceId, rules);
        saved = rules;
      } catch (e) {
        showError((e as Error).message);
        return;
      }
    }
    openDialog({ kind: "publish", instanceId });
  }
</script>

{#if failed}
  <p class="py-8 text-(--danger)">
    Couldn't read the files: {failed}
    <Button size="sm" class="ml-2" onclick={load}>Try again</Button>
  </p>
{:else if !plan}
  <p class="py-8 text-(--text-muted)" role="status">Reading the folder…</p>
{:else}
  <FileTree
    files={plan.files}
    saved={saved ?? plan.rules}
    leftOut={plan.leftOut}
    pending={plan.filesChanged}
    bind:rules
    onsend={send}
  />
{/if}
