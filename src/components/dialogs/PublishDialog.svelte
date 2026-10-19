<script lang="ts">
  import { api } from "$lib/api";
  import { plural } from "$lib/format";
  import { resource } from "$lib/resource.svelte";
  import { instances } from "$lib/stores";
  import type { Id } from "$lib/types";
  import BlockArt from "../BlockArt.svelte";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";
  import UploadPicker, { uploadBytes } from "./UploadPicker.svelte";

  let { instanceId, onclose }: { instanceId: Id; onclose: () => void } =
    $props();

  const instance = $derived(instances.byId(instanceId));
  // What would be published: resolved changes plus files that need an upload decision.
  const plan = resource(() => api.preparePublish(instanceId));

  let changelog = $state("");
  let uploadIds = $state<Id[]>([]);
  let rights = $state(false);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  const changeCount = $derived(
    plan.data
      ? plan.data.changes.added.length +
          plan.data.changes.updated.length +
          plan.data.changes.removed.length
      : 0,
  );
  const overQuota = $derived(
    plan.data
      ? plan.data.storage.usedBytes +
          uploadBytes(plan.data.unknownFiles, uploadIds) >
          plan.data.storage.quotaBytes
      : false,
  );
  // Something has to actually change: a resolved change, or at least one file to upload.
  const nothingToPublish = $derived(
    !!plan.data && changeCount === 0 && uploadIds.length === 0,
  );
  const canSubmit = $derived(
    !submitting &&
      !!plan.data &&
      !nothingToPublish &&
      (uploadIds.length === 0 || rights) &&
      !overQuota,
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
        uploadContentIds: uploadIds,
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
    <BlockArt seed={instanceId} class="size-10 rounded-md" />
  {/snippet}

  {#if plan.loading}
    <p class="py-4 text-[12.5px] text-(--text-muted)">
      Comparing your folder with v{nextVersion - 1}…
    </p>
  {:else if plan.error}
    <p class="py-4 text-[12.5px] text-(--danger)">{plan.error}</p>
  {:else if plan.data}
    {@const c = plan.data.changes}
    <h3 class="mt-1 mb-1 text-xs font-semibold text-(--text-muted)">
      Changes <span class="font-normal text-(--text-faint)">{changeCount}</span>
    </h3>
    {#snippet change(
      symbol: string,
      cls: string,
      name: string,
      version: string,
    )}
      <div
        class="grid grid-cols-[18px_minmax(0,1fr)_auto] items-center gap-3 py-1.5"
      >
        <span
          class="text-center font-(family-name:--font-mono) font-semibold {cls}"
          >{symbol}</span
        >
        <span class="truncate">{name}</span>
        <span class="font-(family-name:--font-mono) text-xs text-(--text-muted)"
          >{version}</span
        >
      </div>
    {/snippet}
    {#each c.added as name (name)}{@render change(
        "+",
        "text-(--online)",
        name,
        "added",
      )}{/each}
    {#each c.updated as u (u.name)}{@render change(
        "↑",
        "text-(--accent)",
        u.name,
        `${u.from} → ${u.to}`,
      )}{/each}
    {#each c.removed as name (name)}{@render change(
        "−",
        "text-(--danger)",
        name,
        "removed",
      )}{/each}
    {#if changeCount === 0}
      <p class="text-[12.5px] text-(--text-muted)">
        No mod changes{plan.data.unknownFiles.length
          ? " — tick a file below to publish it"
          : ""}.
      </p>
    {/if}

    {#if plan.data.unknownFiles.length}
      <UploadPicker
        files={plan.data.unknownFiles}
        storage={plan.data.storage}
        bind:selected={uploadIds}
        bind:rights
      />
    {/if}

    <h3 class="mt-4 mb-1.5 text-xs font-semibold text-(--text-muted)">
      What changed? <span class="font-normal text-(--text-faint)"
        >(optional)</span
      >
    </h3>
    <!-- bind:value works on <textarea> exactly like on <input> -->
    <textarea
      bind:value={changelog}
      maxlength="2000"
      rows="3"
      placeholder="Shown to members when they update"
      class="w-full resize-none rounded-md border border-(--line-2) bg-(--bg) px-3 py-2 leading-snug outline-none focus:border-(--accent)"
    ></textarea>
  {/if}

  {#snippet footer()}
    {#if error}
      <span class="mr-auto text-xs text-(--danger)" role="alert">{error}</span>
    {:else if nothingToPublish}
      <span class="mr-auto text-xs text-(--text-faint)"
        >Nothing to publish yet</span
      >
    {:else if plan.data}
      <span class="mr-auto text-xs text-(--text-faint)">
        {plural(changeCount + uploadIds.length, "change")}
      </span>
    {/if}
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button type="submit" variant="primary" disabled={!canSubmit}>
      {submitting ? "Publishing…" : `Publish v${nextVersion}`}
    </Button>
  {/snippet}
</Dialog>
