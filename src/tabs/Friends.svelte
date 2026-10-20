<script lang="ts" module>
  // Which friends' dropdowns the user opened or closed. Module-level, so it survives
  // leaving the page. A friend with no entry opens by itself when something needs you.
  let expanded = $state<Record<string, boolean>>({});
</script>

<script lang="ts">
  import { plural, timeAgo } from "$lib/format";
  import { social } from "$lib/stores";
  import type { Friend } from "$lib/types";
  import AttentionBadges from "../components/AttentionBadges.svelte";
  import Button from "../components/Button.svelte";
  import Head from "../components/Head.svelte";
  import Icon from "../components/Icon.svelte";
  import Menu from "../components/Menu.svelte";
  import SharedRow, {
    attentionOf,
    byAttention,
  } from "../components/SharedRow.svelte";
  import AddFriendCard from "../components/friends/AddFriendCard.svelte";
  import GroupsCard from "../components/friends/GroupsCard.svelte";
  import RequestsCard from "../components/friends/RequestsCard.svelte";

  // Three sections from one list. Order matters: playing first, then online, then offline.
  const sections = $derived(
    [
      {
        title: "Playing",
        list: social.friends.filter((f) => f.presence.state === "playing"),
      },
      {
        title: "Online",
        list: social.friends.filter((f) => f.presence.state === "online"),
      },
      {
        title: "Offline",
        list: social.friends.filter((f) => f.presence.state === "offline"),
      },
    ].filter((s) => s.list.length > 0),
  );
  const onlineCount = $derived(
    social.friends.filter((f) => f.presence.state !== "offline").length,
  );

  /** What this friend shared with you (or a group you're in), needs-you first. */
  const sharedBy = (f: Friend) =>
    social.shared
      .filter(
        (s) =>
          s.owner?.id === f.user.id &&
          s.myRole !== "owner" &&
          s.myStatus !== "declined",
      )
      .toSorted(byAttention);

  const isOpen = (f: Friend) =>
    expanded[f.user.id] ?? attentionOf(sharedBy(f)).total > 0;
  const toggle = (f: Friend) => (expanded[f.user.id] = !isOpen(f));

  function status(f: Friend) {
    const p = f.presence;
    if (p.state === "playing")
      return p.instanceName ? `Playing ${p.instanceName}` : "Playing Minecraft";
    if (p.state === "online") return "Online in Atlas";
    return `Last seen ${timeAgo(p.updatedAt)}`;
  }

  let error = $state<string | null>(null);

  async function remove(f: Friend, block: boolean) {
    const verb = block ? "Block" : "Remove";
    if (
      !confirm(
        `${verb} ${f.user.displayName}?${block ? " They won't be able to send you requests." : ""}`,
      )
    )
      return;
    error = null;
    try {
      await social.removeFriend(f.user.id, block);
    } catch (e) {
      error = (e as Error).message;
    }
  }
</script>

<div class="mb-6">
  <h1
    class="font-(family-name:--font-display) text-2xl font-semibold tracking-tight"
  >
    Friends
  </h1>
  {#if social.signedIn}
    <p class="mt-1.5 text-(--text-muted)">
      {plural(social.friends.length, "friend")} · {onlineCount} online
    </p>
  {/if}
</div>

{#if !social.signedIn}
  <div class="py-12 text-center text-(--text-muted)">
    <b class="block text-sm text-(--text-primary)">Sign in to add friends</b>
    Friends can see what you're playing and share instances with you.
    <div>
      <Button
        variant="primary"
        class="mt-4"
        onclick={() => social.signIn("minecraft")}>Sign in</Button
      >
    </div>
  </div>
{:else}
  <div
    class="grid grid-cols-[minmax(0,1fr)_320px] items-start gap-10 max-[1150px]:grid-cols-1"
  >
    <div>
      {#if error}<p class="mb-2 text-xs text-(--danger)" role="alert">
          {error}
        </p>{/if}
      {#each sections as section (section.title)}
        <h2
          class="mt-4.5 mb-1 text-xs font-semibold text-(--text-faint) first:mt-0"
        >
          {section.title} · {section.list.length}
        </h2>
        {#each section.list as f (f.user.id)}
          {@const theirs = sharedBy(f)}
          {@const open = theirs.length > 0 && isOpen(f)}
          <div class="rounded-lg {open ? 'mb-2 bg-(--bg-raise)' : ''}">
            <div
              class="group flex items-center gap-3 rounded-lg px-2 py-2.25 {open
                ? 'hover:bg-(--bg-raise-2)'
                : 'hover:bg-(--bg-hover)'}"
            >
              <!-- The row opens the friend's dropdown when they've shared something.
                   Otherwise it's plain text, so it doesn't pretend to be clickable. -->
              <svelte:element
                this={theirs.length ? "button" : "div"}
                role={theirs.length ? undefined : "presentation"}
                aria-expanded={theirs.length ? open : undefined}
                onclick={() => theirs.length && toggle(f)}
                class="flex min-w-0 flex-1 items-center gap-3 text-left"
              >
                <Head
                  user={f.user}
                  size={36}
                  presence={f.presence.state}
                  ring={open ? "var(--bg-raise)" : undefined}
                />
                <span class="min-w-0 flex-1">
                  <b class="font-medium">{f.user.displayName}</b>
                  <span class="text-(--text-faint)">@{f.user.username}</span>
                  <small
                    class="block truncate text-[11.5px] {f.presence.state ===
                    'playing'
                      ? 'text-(--online)'
                      : 'text-(--text-faint)'}"
                  >
                    {status(f)}
                  </small>
                </span>
                {#if theirs.length}
                  <AttentionBadges items={theirs} />
                  <span class="text-xs text-(--text-muted)"
                    >{plural(theirs.length, "shared instance")}</span
                  >
                  <span
                    class="text-(--text-faint) transition-transform {open
                      ? 'rotate-90'
                      : ''}"
                  >
                    <Icon name="chev" class="size-3.5" />
                  </span>
                {/if}
              </svelte:element>
              <!-- Only visible on hover (or while focused, for keyboard users). -->
              <div
                class="opacity-0 group-hover:opacity-100 focus-within:opacity-100"
              >
                <Menu
                  label="More for {f.user.displayName}"
                  items={[
                    {
                      label: "Remove friend",
                      danger: true,
                      onselect: () => remove(f, false),
                    },
                    {
                      label: "Block",
                      danger: true,
                      onselect: () => remove(f, true),
                    },
                  ]}
                />
              </div>
            </div>
            {#if open}
              <div class="border-t border-(--line)">
                {#each theirs as s (s.id)}
                  <SharedRow {s} />
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      {:else}
        <p class="py-12 text-center text-(--text-muted)">
          No friends yet. Send your friend code to someone, or enter theirs.
        </p>
      {/each}
    </div>

    <aside class="flex flex-col gap-4">
      <AddFriendCard />
      <RequestsCard />
      <GroupsCard />
    </aside>
  </div>
{/if}
