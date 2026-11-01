<script lang="ts">
  import { plural, timeAgo } from "$lib/format";
  import { social } from "$lib/stores";
  import { showDone } from "$lib/toasts.svelte";
  import type { Id } from "$lib/types";
  import Button from "../Button.svelte";
  import Head from "../Head.svelte";

  let busy = $state<Id | null>(null);
  let error = $state<string | null>(null);

  async function act(userId: Id, action: () => Promise<unknown>) {
    busy = userId;
    error = null;
    try {
      await action();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = null;
    }
  }
</script>

{#if social.requests.length || social.groupInvites.length}
  <div class="rounded-xl border border-(--line) bg-(--bg-raise) p-4.5">
    <h2 class="mb-1 font-semibold">Requests</h2>
    <div class="divide-y divide-(--line)">
      {#each social.groupInvites as g (g.code)}
        <div class="flex flex-wrap items-center gap-x-2.5 gap-y-2 py-2">
          {#if g.invitedBy}<Head
              user={g.invitedBy}
              size={30}
              ring="var(--bg-raise)"
            />{/if}
          <div class="min-w-40 flex-1">
            <b class="block font-medium"
              >{g.invitedBy?.displayName ?? "A friend"} invited you to {g.groupName}</b
            >
            <small class="text-[13px] text-(--text-faint)"
              >Group · {plural(g.memberCount, "member")} · you get what's shared with
              it</small
            >
          </div>
          <span class="ml-auto flex gap-1.5">
            <Button
              size="sm"
              variant="ghost"
              disabled={busy === g.code}
              onclick={() =>
                act(g.code, () => social.declineGroupInvite(g.code))}
              >Decline</Button
            >
            <Button
              size="sm"
              disabled={busy === g.code}
              onclick={() =>
                act(g.code, async () => {
                  await social.joinGroup(g.code);
                  showDone(`You joined ${g.groupName}.`);
                })}>Join</Button
            >
          </span>
        </div>
      {/each}
      {#each social.incomingRequests as r (r.user.id)}
        <div class="flex flex-wrap items-center gap-x-2.5 gap-y-2 py-2">
          <Head user={r.user} size={30} ring="var(--bg-raise)" />
          <div class="min-w-40 flex-1">
            <b class="block font-medium">{r.user.displayName}</b>
            <small class="text-[13px] text-(--text-faint)"
              >Wants to be friends · {timeAgo(r.createdAt)}</small
            >
          </div>
          <span class="ml-auto flex gap-1.5">
            <Button
              size="sm"
              variant="ghost"
              disabled={busy === r.user.id}
              onclick={() =>
                act(r.user.id, () => social.respondToRequest(r.user.id, false))}
              >Ignore</Button
            >
            <Button
              size="sm"
              disabled={busy === r.user.id}
              onclick={() =>
                act(r.user.id, () => social.respondToRequest(r.user.id, true))}
              >Accept</Button
            >
          </span>
        </div>
      {/each}
      {#each social.outgoingRequests as r (r.user.id)}
        <div class="flex flex-wrap items-center gap-x-2.5 gap-y-2 py-2">
          <Head user={r.user} size={30} ring="var(--bg-raise)" />
          <div class="min-w-40 flex-1">
            <b class="block font-medium">{r.user.displayName}</b>
            <small class="text-[13px] text-(--text-faint)"
              >Request sent · {timeAgo(r.createdAt)}</small
            >
          </div>
          <Button
            size="sm"
            variant="ghost"
            disabled={busy === r.user.id}
            onclick={() =>
              act(r.user.id, () => social.cancelRequest(r.user.id))}
            >Cancel</Button
          >
        </div>
      {/each}
    </div>
    {#if error}<p class="mt-2 text-[13px] text-(--danger)" role="alert">
        {error}
      </p>{/if}
  </div>
{/if}
