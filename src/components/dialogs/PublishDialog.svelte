<script lang="ts">
  import { api } from "$lib/api";
  import { plural } from "$lib/format";
  import { resource } from "$lib/resource.svelte";
  import { instances } from "$lib/stores";
  import type { Id, ShareMode } from "$lib/types";
  import ContentIcon from "../ContentIcon.svelte";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";
  import SharePicker, {
    differences,
    uploadBytes,
    uploadsOf,
  } from "./SharePicker.svelte";

  let { instanceId, onclose }: { instanceId: Id; onclose: () => void } =
    $props();

  const instance = $derived(instances.byId(instanceId));
  // Every file with its setting, plus what disappears (disabled/deleted shared files).
  const plan = resource(() => api.preparePublish(instanceId));

  let modes = $state<Record<Id, ShareMode>>({}); // only what the user changes here
  let rights = $state(false);
  let changelog = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  const diff = $derived(
    plan.data
      ? differences(plan.data.items, modes)
      : { added: [], changed: [] },
  );
  const removedCount = $derived(plan.data?.removed.length ?? 0);
  const total = $derived(
    diff.added.length + diff.changed.length + removedCount,
  );
  const needsRights = $derived(
    !!plan.data && uploadsOf(plan.data.items, modes).length > 0,
  );
  const overQuota = $derived(
    !!plan.data &&
      plan.data.storage.usedBytes + uploadBytes(plan.data.items, modes) >
        plan.data.storage.quotaBytes,
  );
  const tooBig = $derived(
    !!plan.data &&
      uploadsOf(plan.data.items, modes).some(
        (i) => i.sizeBytes > plan.data!.storage.maxFileBytes,
      ),
  );
  const canSubmit = $derived(
    !submitting &&
      !!plan.data &&
      total > 0 &&
      (!needsRights || rights) &&
      !overQuota &&
      !tooBig,
  );
  const nextVersion = $derived((instance?.sync?.latestRevision ?? 0) + 1);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    submitting = true;
    error = null;
    try {
      await api.publishInstance(instanceId, {
        changelog: changelog.trim() || null,
        shareModes: modes,
        confirmedUploadRights: rights,
      });
      await instances.load();
      onclose();
    } catch (err) {
      // e.g. "Someone published a newer version. Update first."
      error = (err as Error).message;
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog
  title="Publish v{nextVersion}"
  description="Everyone with this instance will be asked to update."
  wide
  {onclose}
  onsubmit={submit}
>
  {#snippet header()}
    <ContentIcon
      src={instance?.icon ?? null}
      seed={instanceId}
      class="size-10 rounded-md"
    />
  {/snippet}

  {#if plan.loading}
    <p class="py-4 text-[12.5px] text-(--text-muted)">
      Comparing your folder with v{nextVersion - 1}…
    </p>
  {:else if plan.error}
    <p class="py-4 text-[12.5px] text-(--danger)">{plan.error}</p>
  {:else if plan.data}
    <!-- Plain summary of the effect, updated live as settings change. -->
    <div
      class="mb-3 flex flex-wrap gap-x-4 gap-y-1 rounded-lg bg-(--bg-raise) px-3.5 py-2.5 text-[12.5px]"
    >
      {#if total === 0}
        <span class="text-(--text-muted)"
          >Nothing to publish yet. Change a setting below, or change files in
          your launcher.</span
        >
      {:else}
        {#if diff.added.length}<span
            ><b class="text-(--online)">+{diff.added.length}</b>
            {plural(diff.added.length, "new file")} shared</span
          >{/if}
        {#if diff.changed.length}<span
            ><b class="text-(--accent)">{diff.changed.length}</b>
            {plural(diff.changed.length, "setting")} changed</span
          >{/if}
        {#if removedCount}<span title={plan.data.removed.join(", ")}
            ><b class="text-(--danger)">−{removedCount}</b> removed (disabled in your
            launcher)</span
          >{/if}
      {/if}
    </div>

    <SharePicker
      items={plan.data.items}
      storage={plan.data.storage}
      firstShare={false}
      bind:modes
      bind:rights
    />

    <h3 class="mt-4 mb-1.5 text-xs font-semibold text-(--text-muted)">
      What changed? <span class="font-normal text-(--text-faint)"
        >(optional)</span
      >
    </h3>
    <textarea
      bind:value={changelog}
      maxlength="2000"
      rows="2"
      placeholder="Shown to members when they update"
      class="w-full resize-none rounded-md border border-(--line-2) bg-(--bg) px-3 py-2 leading-snug outline-none focus:border-(--accent)"
    ></textarea>
  {/if}

  {#snippet footer()}
    {#if error}
      <span class="mr-auto text-xs text-(--danger)" role="alert">{error}</span>
    {:else if needsRights && !rights}
      <span class="mr-auto text-xs text-(--warn)"
        >Confirm you can share the uploaded files</span
      >
    {:else if plan.data && total}
      <span class="mr-auto text-xs text-(--text-faint)"
        >{plural(total, "change")}</span
      >
    {/if}
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button type="submit" variant="primary" disabled={!canSubmit}>
      {submitting ? "Publishing…" : `Publish v${nextVersion}`}
    </Button>
  {/snippet}
</Dialog>
