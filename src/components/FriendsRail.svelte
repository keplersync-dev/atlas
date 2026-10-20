<script lang="ts">
  import { go } from "$lib/nav.svelte";
  import { social } from "$lib/stores";
  import { timeAgo } from "$lib/format";
  import type { Friend, Id } from "$lib/types";
  import Button from "./Button.svelte";
  import Head from "./Head.svelte";

  // The friends column on Home. Reads straight from the store, so presence
  // events update it live. Incoming friend requests sit on top: they're about people.

  const online = $derived(
    social.friends.filter((f) => f.presence.state !== "offline"),
  );
  const offline = $derived(
    social.friends.filter((f) => f.presence.state === "offline"),
  );

  function status(f: Friend) {
    const p = f.presence;
    if (p.state === "playing")
      return p.instanceName ? `Playing ${p.instanceName}` : "Playing Minecraft";
    if (p.state === "online") return "Online";
    return `Last seen ${timeAgo(p.updatedAt)}`;
  }

  let busy = $state<Id | null>(null);
  async function respond(userId: Id, accept: boolean) {
    busy = userId;
    try {
      await social.respondToRequest(userId, accept);
    } finally {
      busy = null;
    }
  }
</script>

{#snippet row(f: Friend)}
  <div
    class="-mx-2 flex items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-(--bg-hover)"
  >
    <Head
      user={f.user}
      size={32}
      presence={f.presence.state}
      ring="var(--bg-side)"
    />
    <div class="min-w-0 flex-1">
      <b class="block truncate font-medium">{f.user.displayName}</b>
      <small
        class="block truncate text-[11.5px] {f.presence.state === 'playing'
          ? 'text-(--online)'
          : 'text-(--text-faint)'}"
      >
        {status(f)}
      </small>
    </div>
  </div>
{/snippet}

<div class="mb-3 flex items-baseline justify-between">
  <h2 class="text-[15px] font-semibold">Friends</h2>
  {#if social.signedIn}
    <button
      class="text-xs text-(--accent) hover:underline"
      onclick={() => go({ page: "friends" })}>Add friend</button
    >
  {/if}
</div>

{#if !social.signedIn}
  <p class="text-[12.5px] text-(--text-muted)">
    Sign in to see what your friends are playing and share instances with them.
  </p>
{:else}
  {#if social.incomingRequests.length}
    <div class="mb-4 rounded-lg bg-(--bg-raise) p-3">
      <div class="mb-1.5 text-[11.5px] font-semibold text-(--accent)">
        Friend requests · {social.incomingRequests.length}
      </div>
      {#each social.incomingRequests as r (r.user.id)}
        <div class="flex items-center gap-2.5 py-1.5">
          <Head user={r.user} size={28} ring="var(--bg-raise)" />
          <div class="min-w-0 flex-1">
            <b class="block truncate font-medium">{r.user.displayName}</b>
            <small class="block truncate text-[11px] text-(--text-faint)"
              >@{r.user.username}</small
            >
          </div>
        </div>
        <div class="flex gap-1.5 pb-1">
          <Button
            size="sm"
            variant="primary"
            class="flex-1"
            disabled={busy === r.user.id}
            onclick={() => respond(r.user.id, true)}>Accept</Button
          >
          <Button
            size="sm"
            variant="ghost"
            class="flex-1"
            disabled={busy === r.user.id}
            onclick={() => respond(r.user.id, false)}>Ignore</Button
          >
        </div>
      {/each}
    </div>
  {/if}

  {#if social.friends.length === 0}
    <p class="text-[12.5px] text-(--text-muted)">
      No friends yet. Swap friend codes on the Friends page.
    </p>
  {/if}
  {#if online.length}
    <div class="mt-1 mb-1 text-[11.5px] font-semibold text-(--text-faint)">
      Online · {online.length}
    </div>
    {#each online as f (f.user.id)}{@render row(f)}{/each}
  {/if}
  {#if offline.length}
    <div class="mt-4 mb-1 text-[11.5px] font-semibold text-(--text-faint)">
      Offline · {offline.length}
    </div>
    {#each offline as f (f.user.id)}{@render row(f)}{/each}
  {/if}
{/if}
