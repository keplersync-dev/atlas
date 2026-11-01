<script lang="ts">
  import { api } from "$lib/api";
  import { plural, versionLabel, workLabel } from "$lib/format";
  import { resource } from "$lib/resource.svelte";
  import { instances, social } from "$lib/stores";
  import { showDone } from "$lib/toasts.svelte";
  import type { FileRules, Id, ShareMode } from "$lib/types";
  import ContentIcon from "../ContentIcon.svelte";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";
  import FileTree from "./FileTree.svelte";
  import RightsCheck from "./RightsCheck.svelte";
  import SharePicker, {
    differences,
    names,
    uncheckedOf,
    uploadBytes,
    uploadsOf,
  } from "./SharePicker.svelte";

  let { instanceId, onclose }: { instanceId: Id; onclose: () => void } =
    $props();

  const instance = $derived(instances.byId(instanceId));
  // Reading the folder while the plan loads; uploading and saving while publishing.
  const work = $derived(instances.work[instanceId]);
  // Every file with its setting, plus what disappears (disabled/deleted shared files).
  const plan = resource(() => api.preparePublish(instanceId));

  let modes = $state<Record<Id, ShareMode>>({}); // only what the user changes here
  let rights = $state(false);
  let fileRules = $state<FileRules | null>(null);
  let changelog = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  const diff = $derived(
    plan.data
      ? differences(plan.data.items, modes)
      : { added: [], changed: [] },
  );
  const removedCount = $derived(plan.data?.removed.length ?? 0);
  // The launcher moved to other Minecraft or loader versions: one change.
  const versions = $derived(plan.data?.versions ?? null);
  const total = $derived(
    diff.added.length + diff.changed.length + removedCount + (versions ? 1 : 0),
  );
  const uploads = $derived(plan.data ? uploadsOf(plan.data.items, modes) : []);
  const needsRights = $derived(uploads.length > 0);
  // A file Modrinth and CurseForge couldn't be asked about: sharing waits for them.
  const waiting = $derived(
    !!plan.data && uncheckedOf(plan.data.items, modes).length > 0,
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
      !waiting &&
      !overQuota &&
      !tooBig,
  );
  // Who gets it, by name when it's one person.
  const who = $derived.by(() => {
    const shared = social.shared.find(
      (x) => x.id === instance?.sync?.sharedInstanceId,
    );
    const me = social.session?.user?.id;
    const others = (shared?.members ?? []).filter(
      (m) => m.status === "active" && m.user.id !== me,
    );
    return others.length === 1
      ? others[0].user.displayName
      : others.length
        ? plural(others.length, "person", "people")
        : null;
  });

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
        files: fileRules ?? undefined,
      });
      await instances.load();
      showDone(
        who
          ? `Sent the update to ${who}.`
          : "Update saved. Whoever joins gets it.",
      );
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
  title="Send an update"
  description={who
    ? `Goes to ${who}. Changed files are backed up on their PC first.`
    : "Nobody has joined yet. Whoever joins gets this version."}
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
    <p class="py-4 text-[13px] text-(--text-muted) tabular-nums">
      {work ? workLabel(work) : "Comparing your folder with the last update…"}
    </p>
  {:else if plan.error}
    <p class="py-4 text-[13px] text-(--danger)">{plan.error}</p>
  {:else if plan.data}
    <!-- Plain summary of the effect, updated live as settings change. -->
    <div
      class="mb-3 flex flex-wrap gap-x-4 gap-y-1 rounded-lg bg-(--bg-raise) px-3.5 py-2.5 text-[13px]"
    >
      {#if total === 0}
        <span class="text-(--text-muted)"
          >Nothing to send yet. Change a setting below, or change files in your
          launcher.</span
        >
      {:else}
        {#if diff.added.length}<span
            ><b class="text-(--online)">+{diff.added.length}</b>
            {diff.added.length === 1 ? "new file" : "new files"} shared</span
          >{/if}
        {#if diff.changed.length}<span
            ><b class="text-(--accent)">{diff.changed.length}</b>
            {diff.changed.length === 1 ? "setting" : "settings"} changed</span
          >{/if}
        {#if removedCount}<span title={plan.data.removed.join(", ")}
            ><b class="text-(--danger)">−{removedCount}</b> removed (disabled in your
            launcher)</span
          >{/if}
        {#if versions?.minecraftVersion}<span
            >Minecraft <b>{versions.minecraftVersion.from}</b> →
            <b class="text-(--accent)">{versions.minecraftVersion.to}</b></span
          >{/if}
        {#if versions?.loaderVersion}<span
            >{versionLabel(instance?.loader ?? null, null)}
            <b>{versions.loaderVersion.from}</b> →
            <b class="text-(--accent)">{versions.loaderVersion.to}</b></span
          >{/if}
      {/if}
    </div>

    <h3 class="mb-1.5 text-[13px] font-semibold text-(--text-muted)">
      What changed? <span class="font-normal text-(--text-faint)"
        >(optional)</span
      >
    </h3>
    <textarea
      bind:value={changelog}
      maxlength="2000"
      rows="2"
      placeholder="Friends see this when they update"
      class="mb-3 w-full resize-none rounded-md border border-(--line-2) bg-(--bg) px-3 py-2 leading-snug outline-none focus:border-(--accent)"
    ></textarea>

    <SharePicker
      items={plan.data.items}
      storage={plan.data.storage}
      firstShare={false}
      bind:modes
      onrecheck={plan.reload}
    />
    <FileTree
      files={plan.data.files}
      saved={plan.data.rules}
      leftOut={plan.data.leftOut}
      bind:rules={fileRules}
    />
  {/if}

  {#snippet footer()}
    {#if needsRights}
      <RightsCheck
        bind:checked={rights}
        names={names(uploads)}
        count={uploads.length}
        note={submitting && work ? workLabel(work) : error}
        tone={submitting && work ? "muted" : "danger"}
      />
    {:else if submitting && work}
      <span class="mr-auto text-[13px] text-(--text-muted) tabular-nums"
        >{workLabel(work)}</span
      >
    {:else if error}
      <span class="mr-auto text-[13px] text-(--danger)" role="alert"
        >{error}</span
      >
    {/if}
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button type="submit" variant="primary" disabled={!canSubmit}>
      {submitting ? "Sending…" : "Send update"}
    </Button>
  {/snippet}
</Dialog>
