<script lang="ts">
  import { api } from "$lib/api";
  import { plural, workLabel } from "$lib/format";
  import { resource } from "$lib/resource.svelte";
  import { instances, social } from "$lib/stores";
  import type { Id, ShareMode } from "$lib/types";
  import Button from "../Button.svelte";
  import ContentIcon from "../ContentIcon.svelte";
  import Dialog from "../Dialog.svelte";
  import Head from "../Head.svelte";
  import ConfigPicker from "./ConfigPicker.svelte";
  import RightsCheck from "./RightsCheck.svelte";
  import SharePicker, { uploadBytes, uploadsOf } from "./SharePicker.svelte";

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

  // Only load what this mode needs. Promise.resolve(null) = "nothing to load".
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
    { id: "editor", label: "Publish changes too" },
  ] as const;

  // ── Form state ──
  let friendIds = $state<Id[]>([]);
  let groupIds = $state<Id[]>([]);
  let role = $state<"viewer" | "editor">("viewer");
  let description = $state("");
  // Share setting per file the user changed here; everything else starts "required".
  let modes = $state<Record<Id, ShareMode>>({});
  let rights = $state(false);
  let configExclude = $state<string[] | null>(null);
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
      social.signedIn &&
      (mode === "share"
        ? !!plan.data && (!needsRights || rights) && !overQuota && !tooBig
        : friendIds.length + groupIds.length > 0),
  );

  async function submit(e: SubmitEvent) {
    e.preventDefault(); // stop the browser's default "reload the page" form submit
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
          configExclude: configExclude ?? undefined,
        });
      } else if (sharedId) {
        await api.inviteToSharedInstance(sharedId, {
          friendIds,
          groupIds,
          role,
        });
      }
      await Promise.all([instances.load(), social.load()]);
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
    ? "They'll get the same mods and every version you publish."
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
    <p class="py-4 text-[12.5px] text-(--text-muted)">
      Sign in to share instances with friends.
    </p>
  {:else}
    <h3 class="mt-1 mb-1.5 text-xs font-semibold text-(--text-muted)">
      People
    </h3>
    {#each social.friends as f (f.user.id)}
      {@const status = memberStatus.get(f.user.id)}
      {@const member = !!status}
      <label
        class="-mx-2 flex items-center gap-3 rounded-md px-2 py-1.5 {member
          ? 'opacity-50'
          : 'cursor-pointer hover:bg-(--bg-hover)'}"
      >
        <input
          type="checkbox"
          bind:group={friendIds}
          value={f.user.id}
          disabled={member}
          class="size-3.75 accent-(--accent)"
        />
        <Head user={f.user} size={28} ring="#15181d" />
        <span class="min-w-0 flex-1">
          <b class="block font-medium">{f.user.displayName}</b>
          <small class="block text-[11.5px] text-(--text-faint)"
            >{status === "invited"
              ? "Invite pending"
              : member
                ? "Already a member"
                : `@${f.user.username}`}</small
          >
        </span>
      </label>
    {:else}
      <p class="text-[12.5px] text-(--text-muted)">
        No friends yet. Add some on the Friends page, or share with a group.
      </p>
    {/each}

    {#if groups.data?.length}
      <h3 class="mt-4 mb-1.5 text-xs font-semibold text-(--text-muted)">
        Groups
      </h3>
      {#each groups.data as g (g.id)}
        {@const linked = linkedGroupIds.has(g.id)}
        <label
          class="-mx-2 flex items-center gap-3 rounded-md px-2 py-1.5 {linked
            ? 'opacity-50'
            : 'cursor-pointer hover:bg-(--bg-hover)'}"
        >
          <input
            type="checkbox"
            bind:group={groupIds}
            value={g.id}
            disabled={linked}
            class="size-3.75 accent-(--accent)"
          />
          <span class="flex">
            {#each g.members.slice(0, 3) as m, i (m.user.id)}
              <span class={i ? "-ml-1.5" : ""}
                ><Head user={m.user} size={22} /></span
              >
            {/each}
          </span>
          <span class="min-w-0 flex-1">
            <b class="block font-medium">{g.name}</b>
            <small class="block text-[11.5px] text-(--text-faint)">
              {plural(g.members.length, "member")}{linked
                ? " · already shared"
                : ""}
            </small>
          </span>
        </label>
      {/each}
    {/if}

    <h3 class="mt-4 mb-1.5 text-xs font-semibold text-(--text-muted)">
      They can
    </h3>
    <div
      class="inline-flex gap-0.5 rounded-md bg-(--bg) p-0.75"
      role="group"
      aria-label="Role"
    >
      {#each ROLES as option (option.id)}
        <!-- type="button" so these don't submit the form -->
        <button
          type="button"
          aria-pressed={role === option.id}
          onclick={() => (role = option.id)}
          class="rounded px-3 py-1 text-xs font-medium {role === option.id
            ? 'bg-(--bg-raise-2) text-(--text-primary)'
            : 'text-(--text-muted)'}"
        >
          {option.label}
        </button>
      {/each}
    </div>

    {#if mode === "share"}
      <h3 class="mt-4 mb-1.5 text-xs font-semibold text-(--text-muted)">
        Description <span class="font-normal text-(--text-faint)"
          >(optional)</span
        >
      </h3>
      <input
        bind:value={description}
        maxlength="500"
        placeholder="What's this instance for?"
        class="h-8.5 w-full rounded-md border border-(--line-2) bg-(--bg) px-3 outline-none focus:border-(--accent)"
      />

      <h3 class="mt-4 mb-0.5 text-xs font-semibold text-(--text-muted)">
        What friends get
      </h3>
      {#if plan.loading}
        <p class="text-[12.5px] text-(--text-muted) tabular-nums">
          {work
            ? workLabel(work)
            : "Checking which files friends can download…"}
        </p>
      {:else if plan.error}
        <p class="text-[12.5px] text-(--danger)">{plan.error}</p>
      {:else if plan.data}
        <SharePicker
          items={plan.data.items}
          storage={plan.data.storage}
          firstShare
          bind:modes
          unchecked={plan.data.unchecked}
          onrecheck={plan.reload}
        />
        <ConfigPicker
          configs={plan.data.configs}
          bind:excluded={configExclude}
        />
      {/if}
    {/if}
  {/if}

  {#snippet footer()}
    {#if mode === "share" && needsRights}
      <RightsCheck
        bind:checked={rights}
        note={submitting && work ? workLabel(work) : error}
        tone={submitting && work ? "muted" : "danger"}
      />
    {:else if submitting && work}<span
        class="mr-auto text-xs text-(--text-muted) tabular-nums"
        >{workLabel(work)}</span
      >{:else if error}<span
        class="mr-auto text-xs text-(--danger)"
        role="alert">{error}</span
      >{/if}
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button type="submit" variant="primary" disabled={!canSubmit}>
      {submitting ? "Sharing…" : mode === "share" ? "Share" : "Send invites"}
    </Button>
  {/snippet}
</Dialog>
