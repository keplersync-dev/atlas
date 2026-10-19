<script lang="ts">
  import { go } from "$lib/nav.svelte";
  import { social } from "$lib/stores";
  import { timeAgo } from "$lib/format";
  import type { Friend } from "$lib/types";
  import Head from "./Head.svelte";

  // No props: reads straight from the store, so it updates when presence events arrive.
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
</script>

<!-- A snippet defined inside the component, used twice below. -->
{#snippet row(f: Friend)}
  <div
    class="-mx-2 flex items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-(--bg-hover)"
  >
    <Head user={f.user} size={32} presence={f.presence.state} />
    <div class="min-w-0 flex-1">
      <b class="block font-medium">{f.user.displayName}</b>
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

<aside>
  <div class="mb-2.5 flex items-baseline justify-between">
    <h2 class="text-[13px] font-semibold text-(--text-muted)">Friends</h2>
    {#if social.signedIn}
      <button
        class="text-xs text-(--accent) hover:underline"
        onclick={() => go({ page: "friends" })}>Add friend</button
      >
    {/if}
  </div>

  {#if !social.signedIn}
    <p class="mb-3 text-[12.5px] text-(--text-muted)">
      Sign in to see what your friends are playing and share instances with
      them.
    </p>
  {:else if social.friends.length === 0}
    <p class="text-[12.5px] text-(--text-muted)">
      No friends yet. Swap friend codes on the Friends page.
    </p>
  {:else}
    {#if online.length}
      <div class="mt-3.5 mb-1 text-[11.5px] font-semibold text-(--text-faint)">
        Online · {online.length}
      </div>
      {#each online as f (f.user.id)}
        {@render row(f)}
      {/each}
    {/if}
    {#if offline.length}
      <div class="mt-3.5 mb-1 text-[11.5px] font-semibold text-(--text-faint)">
        Offline
      </div>
      {#each offline as f (f.user.id)}
        {@render row(f)}
      {/each}
    {/if}
  {/if}
</aside>
