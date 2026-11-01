<script lang="ts" module>
  // Which friends' dropdowns the user opened or closed. Module-level, so it survives
  // leaving the page. A friend with no entry opens by itself when something needs you.
  let expanded = $state<Record<string, boolean>>({});
  // Search and filter survive leaving the page too.
  let query = $state("");
  let filter = $state<"all" | "playing" | "online">("all");
</script>

<script lang="ts">
  import { openContextMenu } from "$lib/contextMenu.svelte";
  import { fuzzy } from "$lib/fuzzy";
  import { personItems } from "$lib/people";
  import { plural, timeAgo } from "$lib/format";
  import { social } from "$lib/stores";
  import type { Friend } from "$lib/types";
  import AttentionBadges from "../components/AttentionBadges.svelte";

  import Head from "../components/Head.svelte";
  import Icon from "../components/Icon.svelte";
  import Menu from "../components/Menu.svelte";
  import SharedRow, {
    attentionOf,
    byAttention,
  } from "../components/SharedRow.svelte";
  import GroupsCard from "../components/friends/GroupsCard.svelte";
  import RequestsCard from "../components/friends/RequestsCard.svelte";
  import SignInButtons from "../components/SignInButtons.svelte";
  import YouCard from "../components/friends/YouCard.svelte";

  const FILTERS = [
    { key: "all", label: "All", test: () => true },
    {
      key: "playing",
      label: "Playing",
      test: (f) => f.presence.state === "playing",
    },
    {
      key: "online",
      label: "Online",
      test: (f) => f.presence.state !== "offline",
    },
  ] as const satisfies readonly {
    key: typeof filter;
    label: string;
    test: (f: Friend) => boolean;
  }[];

  const matchesQuery = (f: Friend) =>
    fuzzy(query, f.user.displayName, f.user.username) !== null;
  const shown = $derived(
    social.friends.filter(
      (f) => matchesQuery(f) && FILTERS.find((x) => x.key === filter)!.test(f),
    ),
  );

  // Three sections from one list. Order matters: playing first, then online, then offline.
  const sections = $derived(
    [
      {
        title: "Playing",
        list: shown.filter((f) => f.presence.state === "playing"),
      },
      {
        title: "Online",
        list: shown.filter((f) => f.presence.state === "online"),
      },
      {
        title: "Offline",
        list: shown.filter((f) => f.presence.state === "offline"),
      },
    ].filter((s) => s.list.length > 0),
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
</script>

{#if !social.signedIn}
  <div class="py-12 text-center text-(--text-muted)">
    <b class="block text-sm text-(--text-primary)">Sign in to add friends</b>
    Friends can see what you're playing and share instances with you.
    <SignInButtons />
  </div>
{:else}
  <YouCard />
  <!-- Requests and group invites: a strip above both columns, only when there are any. -->
  <div class="mt-4 empty:hidden"><RequestsCard /></div>
  <!-- Friends and groups get equal room side by side; stacked when that's too narrow. -->
  <div
    class="mt-6 grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)] items-start gap-8 max-[1400px]:grid-cols-1"
  >
    <!-- The section card the Shared page uses, the same as Groups beside it. -->
    <section class="rounded-xl border border-(--line) bg-(--bg-raise)">
      <header
        class="flex items-center gap-3 border-b border-(--line) px-4 py-3.5"
      >
        <span
          class="grid size-8 place-items-center rounded-lg bg-(--bg-raise-2) text-(--text-muted)"
        >
          <Icon name="users" class="size-4" />
        </span>
        <span class="min-w-0 flex-1">
          <b class="font-semibold">Friends</b>
          <span class="ml-1 text-(--text-faint)">{social.friends.length}</span>
          <small class="block text-[13px] text-(--text-faint)"
            >See what they're playing and what they share with you.</small
          >
        </span>
      </header>
      <div
        class="flex flex-wrap items-center gap-x-4 gap-y-3 border-b border-(--line) px-4 py-3"
      >
        <label class="relative block max-w-64 min-w-36 flex-1">
          <span
            class="pointer-events-none absolute top-1/2 left-2.5 flex -translate-y-1/2 text-(--text-faint)"
          >
            <Icon name="search" class="size-3.5" />
          </span>
          <input
            bind:value={query}
            placeholder="Search friends"
            aria-label="Search friends"
            class="h-8 w-full rounded-md border border-(--line-2) bg-(--bg) pr-2.5 pl-8 text-[13px] placeholder:text-(--text-faint) focus:border-(--accent) focus:outline-none"
          />
        </label>
        <div class="flex gap-1.5" role="radiogroup" aria-label="Filter">
          {#each FILTERS as f (f.key)}
            <button
              role="radio"
              aria-checked={filter === f.key}
              onclick={() => (filter = f.key)}
              class="h-7 rounded-full px-3 text-[13px] {filter === f.key
                ? 'bg-(--accent)/15 font-semibold text-(--accent)'
                : 'text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)'}"
            >
              {f.label}
              <span class="ml-0.5 opacity-70"
                >{social.friends.filter(f.test).length}</span
              >
            </button>
          {/each}
        </div>
      </div>
      <div class="px-2 pt-1 pb-2">
        {#each sections as section (section.title)}
          <h2
            class="mt-2 mb-0.5 px-2 pt-1 text-[13px] font-semibold text-(--text-faint)"
          >
            {section.title} · {section.list.length}
          </h2>
          {#each section.list as f (f.user.id)}
            {@const theirs = sharedBy(f)}
            {@const open = theirs.length > 0 && isOpen(f)}
            <!-- svelte-ignore a11y_no_static_element_interactions (right-click repeats the ⋯ menu, which keyboards reach) -->
            <div
              class="rounded-lg {open ? 'my-1 bg-(--bg)' : ''}"
              oncontextmenu={(e) => openContextMenu(e, personItems(f.user))}
            >
              <div
                class="group flex items-center gap-3 rounded-lg px-2 py-2.25 hover:bg-(--bg-hover)"
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
                    ring={open ? "var(--bg)" : "var(--bg-raise)"}
                  />
                  <span class="min-w-0 flex-1">
                    <b class="font-medium">{f.user.displayName}</b>
                    <span class="text-(--text-faint)">@{f.user.username}</span>
                    <small
                      class="block truncate text-[13px] {f.presence.state ===
                      'playing'
                        ? 'text-(--online)'
                        : 'text-(--text-faint)'}"
                    >
                      {status(f)}
                    </small>
                  </span>
                  {#if theirs.length}
                    <AttentionBadges items={theirs} />
                    <span class="text-[13px] text-(--text-muted)"
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
                    items={personItems(f.user)}
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
          <p class="px-2 py-10 text-center text-[13px] text-(--text-muted)">
            {#if !social.friends.length}
              No friends yet. Send your friend code to someone, or paste theirs
              above.
            {:else if query.trim()}
              Nobody matches “{query.trim()}”.
            {:else if filter === "playing"}
              None of your friends are playing right now.
            {:else}
              None of your friends are online right now.
            {/if}
          </p>
        {/each}
      </div>
    </section>

    <GroupsCard />
  </div>
{/if}
