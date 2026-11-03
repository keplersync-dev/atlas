<script lang="ts">
  import { api } from "$lib/api";
  import { plural, workLabel } from "$lib/format";
  import { resource } from "$lib/resource.svelte";
  import { showDone } from "$lib/toasts.svelte";
  import { instances, social } from "$lib/stores";
  import type { FileRules, Id, ShareMode } from "$lib/types";
  import Button from "../Button.svelte";
  import ContentIcon from "../ContentIcon.svelte";
  import Dialog from "../Dialog.svelte";
  import PeoplePicker from "../PeoplePicker.svelte";
  import FileTree from "./FileTree.svelte";
  import RightsCheck from "./RightsCheck.svelte";
  import SharePicker, { uploadCheck } from "./SharePicker.svelte";

  // "share": first share of an instance that's only on this PC (creates revision 1).
  // "invite": add people to an instance that's already shared.
  let {
    instanceId,
    mode,
    onclose,
  }: { instanceId: Id; mode: "share" | "invite"; onclose: () => void } =
    $props();

  const instance = $derived(instances.byId(instanceId));
  // Reading the folder while the plan loads; uploading and saving while sharing.
  const work = $derived(instances.work[instanceId]);
  const sharedId = $derived(instance?.sync?.sharedInstanceId ?? null);

  // Only load what this mode needs.
  const plan = resource(() =>
    mode === "share" ? api.preparePublish(instanceId) : Promise.resolve(null),
  );
  const shared = resource(() =>
    mode === "invite" && sharedId
      ? api.getSharedInstance(sharedId)
      : Promise.resolve(null),
  );
  const groups = resource(() => api.listGroups());

  const ROLES = [
    { id: "viewer", label: "Get updates" },
    { id: "editor", label: "Send updates too" },
  ] as const;

  // ── Form state ──
  let friendIds = $state<Id[]>([]);
  let groupIds = $state<Id[]>([]);
  let role = $state<"viewer" | "editor">("viewer");
  let description = $state("");
  // Share setting per file the user changed here; everything else starts "required".
  let modes = $state<Record<Id, ShareMode>>({});
  let rights = $state(false);
  let fileRules = $state<FileRules | null>(null);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // People who already have it (invite mode) can't be picked again.
  // People already in, or with an invite they haven't answered. Someone who declined
  // can be invited again.
  const memberStatus = $derived(
    new Map(
      (shared.data?.members ?? [])
        .filter((m) => m.status !== "declined")
        .map((m) => [m.user.id, m.status]),
    ),
  );
  const linkedGroupIds = $derived(
    new Set(shared.data?.groups.map((g) => g.group.id) ?? []),
  );

  const check = $derived(plan.data ? uploadCheck(plan.data, modes) : null);
  const uploads = $derived(check?.uploads ?? []);

  const canSubmit = $derived(
    !submitting &&
      social.signedIn &&
      (mode === "share"
        ? !!check && !check.blocked && (!uploads.length || rights)
        : friendIds.length + groupIds.length > 0),
  );

  // Who's picked, in words, for the button and the confirmation: a name when it's one,
  // otherwise "3 people and 2 groups".
  const picked = $derived.by(() => {
    const people = social.friends.filter((f) => friendIds.includes(f.user.id));
    const picks = (groups.data ?? []).filter((g) => groupIds.includes(g.id));
    if (people.length + picks.length === 1)
      return people[0]?.user.displayName ?? picks[0].name;
    return (
      [
        people.length && plural(people.length, "person", "people"),
        picks.length && plural(picks.length, "group"),
      ]
        .filter(Boolean)
        .join(" and ") || null
    );
  });
  // Invite mode with everyone already in: nothing to pick.
  const pickable = $derived(
    social.friends.filter((f) => !memberStatus.get(f.user.id)).length +
      (groups.data?.filter((g) => !linkedGroupIds.has(g.id)).length ?? 0),
  );

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    submitting = true;
    error = null;
    try {
      if (mode === "share") {
        await api.shareInstance(instanceId, {
          description: description.trim() || null,
          friendIds,
          groupIds,
          role,
          shareModes: modes,
          confirmedUploadRights: rights,
          files: fileRules ?? undefined,
        });
      } else if (sharedId) {
        await api.inviteToSharedInstance(sharedId, {
          friendIds,
          groupIds,
          role,
        });
      }
      await Promise.all([instances.load(), social.load()]);
      const name = instance?.name ?? "It";
      showDone(
        mode === "invite"
          ? `Invited ${picked} to ${name}.`
          : picked
            ? `${name} is shared with ${picked}.`
            : `${name} is shared. Invite friends from its page.`,
      );
      onclose();
    } catch (err) {
      error = (err as Error).message;
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog
  title={mode === "share"
    ? `Share ${instance?.name ?? "instance"}`
    : `Invite to ${instance?.name ?? "instance"}`}
  description={mode === "share"
    ? "They get the same mods and every update you send."
    : "New members get the current version and every update after it."}
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

  {#if !social.signedIn}
    <p class="py-4 text-[13px] text-(--text-muted)">
      Sign in to share packs with friends.
    </p>
  {:else}
    <h3 class="mt-1 mb-1.5 text-[13px] font-semibold text-(--text-muted)">
      Share with
    </h3>
    <!-- Those who already have it are ticked and greyed, so there's nothing to pick. -->
    <PeoplePicker
      friends={social.friends}
      groups={groups.data ?? []}
      bind:friendIds
      bind:groupIds
      friendTaken={(id) => {
        const status = memberStatus.get(id);
        return status === "invited"
          ? "Invite pending"
          : status
            ? "Already a member"
            : null;
      }}
      groupTaken={(id) => (linkedGroupIds.has(id) ? "Already shared" : null)}
      empty="No friends yet. Add some on the Friends page, or share with a group."
    />

    {#if mode === "invite" && social.friends.length && !pickable}
      <p class="mt-3 text-[13px] text-(--text-muted)">
        Everyone on your friends list already has it. Add friends on the Friends
        page to invite more.
      </p>
    {:else}
      <!-- What they can do and the description share a row; the description wraps
           under it when the dialog is narrow. -->
      <div class="mt-4 flex flex-wrap items-end gap-x-6 gap-y-4">
        <div class="shrink-0">
          <h3 class="mb-1.5 text-[13px] font-semibold text-(--text-muted)">
            They can
          </h3>
          <div
            class="inline-flex h-8.5 items-center gap-0.5 rounded-md bg-(--bg) p-0.75"
            role="group"
            aria-label="Role"
          >
            {#each ROLES as option (option.id)}
              <button
                type="button"
                aria-pressed={role === option.id}
                onclick={() => (role = option.id)}
                class="rounded px-3 py-1 text-[13px] font-medium {role ===
                option.id
                  ? 'bg-(--bg-raise-2) text-(--text-primary)'
                  : 'text-(--text-muted)'}"
              >
                {option.label}
              </button>
            {/each}
          </div>
        </div>
        {#if mode === "share"}
          <div class="min-w-56 flex-1">
            <h3 class="mb-1.5 text-[13px] font-semibold text-(--text-muted)">
              Description <span class="font-normal text-(--text-faint)"
                >(optional)</span
              >
            </h3>
            <input
              bind:value={description}
              maxlength="500"
              placeholder="What's this pack for?"
              class="h-8.5 w-full rounded-md border border-(--line-2) bg-(--bg) px-3 outline-none focus:border-(--accent)"
            />
          </div>
        {/if}
      </div>
    {/if}

    {#if mode === "share"}
      <h3 class="mt-4 mb-0.5 text-[13px] font-semibold text-(--text-muted)">
        What friends get
      </h3>
      {#if plan.loading}
        <p class="text-[13px] text-(--text-muted) tabular-nums">
          {work
            ? workLabel(work)
            : "Checking which files friends can download…"}
        </p>
      {:else if plan.error}
        <p class="text-[13px] text-(--danger)">{plan.error}</p>
      {:else if plan.data}
        <SharePicker
          items={plan.data.items}
          storage={plan.data.storage}
          firstShare
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
    {/if}
  {/if}

  {#snippet footer()}
    {@const sharing = mode === "share" && social.signedIn}
    <RightsCheck
      bind:checked={rights}
      uploads={sharing ? uploads : []}
      {submitting}
      {work}
      {error}
      hint={sharing && !picked
        ? "Nobody picked yet. You can invite friends later."
        : null}
    />
    {#if mode === "invite" && social.friends.length && !pickable}
      <Button onclick={onclose}>Close</Button>
    {:else}
      <Button variant="ghost" onclick={onclose}>Cancel</Button>
      <Button type="submit" variant="primary" disabled={!canSubmit}>
        {submitting
          ? "Sharing…"
          : mode === "invite"
            ? "Send invites"
            : picked
              ? `Share with ${picked}`
              : "Share"}
      </Button>
    {/if}
  {/snippet}
</Dialog>
