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
  import { publishChanges } from "$lib/publishChanges";
  import SharePicker, { names, uploadCheck } from "./SharePicker.svelte";

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

  const changes = $derived(
    plan.data ? publishChanges(plan.data, modes, fileRules) : null,
  );
  const total = $derived(changes?.total ?? 0);
  // The launcher moved to other Minecraft or loader versions: one change.
  const versions = $derived(plan.data?.versions ?? null);
  const check = $derived(plan.data ? uploadCheck(plan.data, modes) : null);
  const uploads = $derived(check?.uploads ?? []);
  const canSubmit = $derived(
    !submitting &&
      !!check &&
      total > 0 &&
      !check.blocked &&
      (!uploads.length || rights),
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
        {@const { added, changed, updated, ticks } = changes!}
        {@const others = plan.data.filesChanged.length}
        {@const removed = plan.data.removed.length}
        {#if added.length}<span
            ><b class="text-(--online)">+{added.length}</b>
            {added.length === 1 ? "new file" : "new files"} shared</span
          >{/if}
        {#if changed.length}<span
            ><b class="text-(--accent)">{changed.length}</b>
            {changed.length === 1 ? "setting" : "settings"} changed</span
          >{/if}
        {#if updated.length}<span title={names(updated)}
            ><b class="text-(--accent)">{updated.length}</b>
            {updated.length === 1 ? "file" : "files"} updated</span
          >{/if}
        {#if others}<span title={plan.data.filesChanged.join(", ")}
            ><b class="text-(--accent)">{others}</b>
            {others === 1 ? "other file" : "other files"} changed</span
          >{/if}
        {#if ticks}<span>Shared files picked differently</span>{/if}
        {#if removed}<span title={plan.data.removed.join(", ")}
            ><b class="text-(--danger)">−{removed}</b> removed (disabled in your launcher)</span
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
    <RightsCheck bind:checked={rights} {uploads} {submitting} {work} {error} />
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button type="submit" variant="primary" disabled={!canSubmit}>
      {submitting ? "Sending…" : "Send update"}
    </Button>
  {/snippet}
</Dialog>
