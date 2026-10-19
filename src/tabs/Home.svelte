<script lang="ts">
  import { instances, social } from "$lib/stores";
  import { go } from "$lib/nav.svelte";
  import { openDialog } from "$lib/dialogs.svelte";
  import { fileSize, plural, timeAgo, versionLabel } from "$lib/format";
  import AttentionRow from "../components/AttentionRow.svelte";
  import BlockArt from "../components/BlockArt.svelte";
  import Button from "../components/Button.svelte";
  import FriendsRail from "../components/FriendsRail.svelte";
  import Head from "../components/Head.svelte";
  import InstanceTile from "../components/InstanceTile.svelte";

  const MAX_INVITES = 3;

  // Which row's button is running, so only that one shows as busy.
  let busy = $state<string | null>(null);
  let actionError = $state<string | null>(null);

  async function run(key: string, action: () => Promise<unknown>) {
    busy = key;
    actionError = null;
    try {
      await action();
    } catch (e) {
      actionError = (e as Error).message;
    } finally {
      busy = null;
    }
  }

  const invites = $derived(social.invites.slice(0, MAX_INVITES));
  const moreInvites = $derived(social.invites.length - invites.length);
  const nothingWaiting = $derived(
    instances.needingAttention.length === 0 &&
      social.invites.length === 0 &&
      social.incomingRequests.length === 0,
  );
</script>

{#if !instances.loaded}
  <p class="text-(--text-muted)">Loading…</p>
{:else if instances.error}
  <div class="text-(--danger)">
    Couldn't load your instances: {instances.error}
    <Button size="sm" class="ml-2" onclick={() => instances.load()}
      >Try again</Button
    >
  </div>
{:else if instances.list.length === 0}
  <!-- First run. The track dialog comes later; for now this explains what's next. -->
  <div class="max-w-xl pt-10">
    <h1
      class="font-(family-name:--font-display) text-3xl font-semibold tracking-tight"
    >
      Keep your friends on the same mods.
    </h1>
    <p class="mt-2.5 text-sm text-(--text-muted)">
      Atlas works alongside the launcher you already use. Track an instance from
      Prism, the Modrinth App or CurseForge to share it or receive updates for
      it.
    </p>
    <Button
      variant="primary"
      size="lg"
      class="mt-6"
      onclick={() => openDialog({ kind: "track" })}>Choose instances</Button
    >
  </div>
{:else}
  <div
    class="grid grid-cols-[minmax(0,1fr)_264px] gap-10 max-[1150px]:grid-cols-1"
  >
    <div>
      <!-- ── Needs you ───────────────────────────────────────────── -->
      <section>
        <div class="mb-2.5 flex items-baseline justify-between">
          <h2 class="text-[13px] font-semibold text-(--text-muted)">
            Needs you
          </h2>
          {#if moreInvites > 0}
            <button
              class="text-xs text-(--accent) hover:underline"
              onclick={() => go({ page: "shared" })}
            >
              {plural(moreInvites, "more invite")}
            </button>
          {/if}
        </div>

        <div class="divide-y divide-(--line) rounded-xl bg-(--bg-raise)">
          {#if nothingWaiting}
            <p class="px-4 py-3.5 text-(--text-muted)">
              You're all caught up. Everything you share is in sync.
            </p>
          {/if}

          {#each instances.needingAttention as inst (inst.id)}
            {@const s = inst.sync!}
            {#if s.state === "local_changes"}
              <AttentionRow
                kicker="Your changes"
                detail="{plural(
                  s.localChangeCount,
                  'change',
                )} not published yet · on v{s.syncedRevision}"
              >
                {#snippet art()}<BlockArt
                    seed={inst.id}
                    class="size-10 rounded-md"
                  />{/snippet}
                <b>{inst.name}</b> changed since you last published
                {#snippet actions()}
                  <Button
                    variant="ghost"
                    onclick={() => go({ page: "instance", id: inst.id })}
                    >See changes</Button
                  >
                  <Button
                    variant="primary"
                    onclick={() =>
                      openDialog({ kind: "publish", instanceId: inst.id })}
                    >Publish</Button
                  >
                {/snippet}
              </AttentionRow>
            {:else}
              <AttentionRow
                kicker="Update"
                tone="warn"
                detail="v{s.syncedRevision} → v{s.latestRevision}{s.state ===
                'conflict'
                  ? ' · would overwrite your local changes'
                  : ''}"
              >
                {#snippet art()}<BlockArt
                    seed={inst.id}
                    class="size-10 rounded-md"
                  />{/snippet}
                <b>{s.owner?.displayName ?? "The owner"}</b> published v{s.latestRevision}
                of <b>{inst.name}</b>
                {#snippet actions()}
                  <Button
                    variant="warn"
                    onclick={() => go({ page: "review", id: inst.id })}
                    >Review update</Button
                  >
                {/snippet}
              </AttentionRow>
            {/if}
          {/each}

          {#each invites as invite (invite.id)}
            <AttentionRow
              kicker="Invite"
              detail="{versionLabel(
                invite.loader,
                invite.minecraftVersion,
              )} · {plural(invite.contentCount, 'item')} · {fileSize(
                invite.sizeBytes,
              )}"
            >
              {#snippet art()}<BlockArt
                  seed={invite.id}
                  class="size-10 rounded-md"
                />{/snippet}
              <b>{(invite.invitedBy ?? invite.owner)?.displayName}</b> shared
              <b>{invite.name}</b>
              {invite.myViaGroup
                ? `with ${invite.myViaGroup.name}`
                : "with you"}
              {#snippet actions()}
                <Button
                  variant="ghost"
                  disabled={busy === invite.id}
                  onclick={() =>
                    run(invite.id, () => social.declineInvite(invite.id))}
                  >Decline</Button
                >
                <Button
                  onclick={() =>
                    openDialog({ kind: "accept", sharedInstanceId: invite.id })}
                  >Add to a launcher</Button
                >
              {/snippet}
            </AttentionRow>
          {/each}

          {#each social.incomingRequests as request (request.user.id)}
            <AttentionRow
              kicker="Friend request"
              detail="@{request.user.username} · {timeAgo(request.createdAt)}"
            >
              {#snippet art()}<Head
                  user={request.user}
                  size={40}
                  ring="var(--bg-raise)"
                />{/snippet}
              <b>{request.user.displayName}</b> wants to be friends
              {#snippet actions()}
                <Button
                  variant="ghost"
                  disabled={busy === request.user.id}
                  onclick={() =>
                    run(request.user.id, () =>
                      social.respondToRequest(request.user.id, false),
                    )}>Ignore</Button
                >
                <Button
                  disabled={busy === request.user.id}
                  onclick={() =>
                    run(request.user.id, () =>
                      social.respondToRequest(request.user.id, true),
                    )}>Accept</Button
                >
              {/snippet}
            </AttentionRow>
          {/each}
        </div>

        {#if actionError}
          <p class="mt-2 text-xs text-(--danger)" role="alert">{actionError}</p>
        {/if}
      </section>

      <!-- ── Recently played ─────────────────────────────────────── -->
      <section class="mt-8">
        <h2 class="mb-2.5 text-[13px] font-semibold text-(--text-muted)">
          Recently played
        </h2>
        <div
          class="grid grid-cols-[repeat(auto-fill,minmax(176px,1fr))] gap-x-3.5 gap-y-4"
        >
          {#each instances.recentlyPlayed as inst (inst.id)}
            <InstanceTile instance={inst} />
          {/each}
        </div>
      </section>
    </div>

    <FriendsRail />
  </div>
{/if}
