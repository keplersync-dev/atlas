<script lang="ts">
  import { openContextMenu } from "$lib/contextMenu.svelte";
  import { go } from "$lib/nav.svelte";
  import { personItems, presenceText } from "$lib/people";
  import { social } from "$lib/stores";
  import { showError } from "$lib/toasts.svelte";
  import type { Friend, Id } from "$lib/types";
  import Button from "./Button.svelte";
  import Head from "./Head.svelte";
  import PersonName from "./PersonName.svelte";
  import Menu from "./Menu.svelte";

  // The friends column on Home. Reads straight from the store, so presence
  // events update it live. Incoming friend requests sit on top: they're about people.

  const online = $derived(
    social.friends.filter((f) => f.presence.state !== "offline"),
  );
  const offline = $derived(
    social.friends.filter((f) => f.presence.state === "offline"),
  );

  let busy = $state<Id | null>(null);
  async function respond(userId: Id, accept: boolean) {
    busy = userId;
    try {
      await social.respondToRequest(userId, accept);
    } catch (e) {
      showError((e as Error).message);
    } finally {
      busy = null;
    }
  }

  // Online friends always show; a long offline list is cut short until asked.
  const OFFLINE_SHOWN = 8;
  let allOffline = $state(false);
</script>

{#snippet row(f: Friend)}
  <!-- svelte-ignore a11y_no_static_element_interactions (right-click repeats the ⋯ menu, which keyboards reach) -->
  <div
    class="group -mx-2 flex items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-(--bg-hover)"
    oncontextmenu={(e) => openContextMenu(e, personItems(f.user))}
  >
    <Head
      user={f.user}
      size={32}
      presence={f.presence.state}
      ring="var(--bg-side)"
    />
    <div class="min-w-0 flex-1">
      <PersonName user={f.user} />
      <small
        class="block truncate text-[13px] {f.presence.state === 'playing'
          ? 'text-(--online)'
          : 'text-(--text-faint)'}"
      >
        {presenceText(f.presence)}
      </small>
    </div>
    <!-- Shown on hover, or while focused for keyboards, like on the Friends page. -->
    <div class="opacity-0 group-hover:opacity-100 focus-within:opacity-100">
      <Menu
        label="More for {f.user.displayName} (@{f.user.username})"
        items={personItems(f.user)}
        triggerClass="grid size-7 place-items-center rounded-md text-(--text-muted) hover:bg-(--bg-raise-2) hover:text-(--text-primary)"
      />
    </div>
  </div>
{/snippet}

<div class="mb-3 flex items-baseline justify-between">
  <h2 class="text-[15px] font-semibold">Friends</h2>
  {#if social.signedIn}
    <button
      class="tap text-[13px] text-(--accent) hover:underline"
      onclick={() => go({ page: "friends" })}>Add friend</button
    >
  {/if}
</div>

{#if !social.signedIn}
  <p class="text-[13px] text-(--text-muted)">
    Sign in to see what your friends are playing and share packs with them.
  </p>
{:else}
  {#if social.incomingRequests.length}
    <div class="mb-4 rounded-lg bg-(--bg-raise) p-3">
      <div class="mb-1.5 text-[13px] font-semibold text-(--accent)">
        Friend requests · {social.incomingRequests.length}
      </div>
      {#each social.incomingRequests as r (r.user.id)}
        <div class="flex items-center gap-2.5 py-1.5">
          <Head user={r.user} size={28} ring="var(--bg-raise)" />
          <div class="min-w-0 flex-1">
            <PersonName user={r.user} />
            <small class="block truncate text-[13px] text-(--text-faint)"
              >Wants to be friends</small
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
    <p class="text-[13px] text-(--text-muted)">
      No friends yet. Add them by Minecraft name on the Friends page.
    </p>
  {/if}
  {#if online.length}
    <div class="mt-1 mb-1 text-[13px] font-semibold text-(--text-faint)">
      Online · {online.length}
    </div>
    {#each online as f (f.user.id)}{@render row(f)}{/each}
  {/if}
  {#if offline.length}
    <div class="mt-4 mb-1 text-[13px] font-semibold text-(--text-faint)">
      Offline · {offline.length}
    </div>
    {#each allOffline ? offline : offline.slice(0, OFFLINE_SHOWN) as f (f.user.id)}{@render row(
        f,
      )}{/each}
    {#if offline.length > OFFLINE_SHOWN}
      <button
        type="button"
        class="mt-1 w-full rounded-md px-2 py-1 text-left text-[13px] text-(--accent) hover:bg-(--bg-hover)"
        onclick={() => (allOffline = !allOffline)}
        >{allOffline ? "Show fewer" : `Show all ${offline.length}`}</button
      >
    {/if}
  {/if}
{/if}
