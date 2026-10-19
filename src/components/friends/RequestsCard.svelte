<script lang="ts">
  import { timeAgo } from "$lib/format";
  import { social } from "$lib/stores";
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

{#if social.requests.length}
  <div class="rounded-xl bg-(--bg-raise) p-4.5">
    <h2 class="mb-1 font-semibold">Requests</h2>
    <div class="divide-y divide-(--line)">
      {#each social.incomingRequests as r (r.user.id)}
        <div class="flex items-center gap-2.5 py-2">
          <Head user={r.user} size={30} ring="var(--bg-raise)" />
          <div class="min-w-0 flex-1">
            <b class="block font-medium">{r.user.displayName}</b>
            <small class="text-[11.5px] text-(--text-faint)"
              >Wants to be friends · {timeAgo(r.createdAt)}</small
            >
          </div>
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
        </div>
      {/each}
      {#each social.outgoingRequests as r (r.user.id)}
        <div class="flex items-center gap-2.5 py-2">
          <Head user={r.user} size={30} ring="var(--bg-raise)" />
          <div class="min-w-0 flex-1">
            <b class="block font-medium">{r.user.displayName}</b>
            <small class="text-[11.5px] text-(--text-faint)"
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
    {#if error}<p class="mt-2 text-xs text-(--danger)" role="alert">
        {error}
      </p>{/if}
  </div>
{/if}
