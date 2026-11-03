<script lang="ts">
  import { fuzzyFilter } from "$lib/fuzzy";
  import { plural } from "$lib/format";
  import type { Friend, Group, Id } from "$lib/types";
  import Head from "./Head.svelte";
  import Icon from "./Icon.svelte";
  import SearchBox from "./SearchBox.svelte";

  // Ticks friends (and groups, when given) out of lists that can run to hundreds: one
  // search for both, a tab each, a list that scrolls in place, and the picks as chips
  // so they stay in view whichever tab is open.
  let {
    friends,
    groups = [],
    friendIds = $bindable([]),
    groupIds = $bindable([]),
    friendTaken = () => null,
    groupTaken = () => null,
    empty,
  }: {
    friends: Friend[];
    groups?: Group[];
    friendIds?: Id[];
    groupIds?: Id[];
    /** Why a friend can't be picked ("Already a member"), shown ticked and greyed. */
    friendTaken?: (id: Id) => string | null;
    groupTaken?: (id: Id) => string | null;
    /** Said when there are no friends at all. */
    empty: string;
  } = $props();

  let query = $state("");
  let tab = $state<"people" | "groups">("people");

  const shownFriends = $derived(
    fuzzyFilter(friends, query, (f) => [f.user.displayName, f.user.username]),
  );
  const shownGroups = $derived(fuzzyFilter(groups, query, (g) => [g.name]));
  // A search that only finds groups shows them, rather than an empty People tab.
  const showing = $derived(
    !groups.length
      ? "people"
      : query && !shownFriends.length && shownGroups.length
        ? "groups"
        : query && !shownGroups.length && shownFriends.length
          ? "people"
          : tab,
  );

  const pickedFriends = $derived(
    friends.filter((f) => friendIds.includes(f.user.id)),
  );
  const pickedGroups = $derived(groups.filter((g) => groupIds.includes(g.id)));

  // Picks show as chips up to a couple of rows' worth; the rest are a "+N more".
  const CHIPS_SHOWN = 8;
  let allChips = $state(false);
  const shownGroupChips = $derived(
    allChips ? pickedGroups : pickedGroups.slice(0, CHIPS_SHOWN),
  );
  const shownFriendChips = $derived(
    allChips
      ? pickedFriends
      : pickedFriends.slice(0, Math.max(0, CHIPS_SHOWN - pickedGroups.length)),
  );
  const hiddenChips = $derived(
    pickedGroups.length +
      pickedFriends.length -
      shownGroupChips.length -
      shownFriendChips.length,
  );
  const pickedSummary = $derived(
    [
      pickedFriends.length && plural(pickedFriends.length, "person", "people"),
      pickedGroups.length && plural(pickedGroups.length, "group"),
    ]
      .filter(Boolean)
      .join(" and "),
  );

  const TABS = $derived([
    { id: "people" as const, label: "People", count: shownFriends.length },
    { id: "groups" as const, label: "Groups", count: shownGroups.length },
  ]);
</script>

{#if !friends.length && !groups.length}
  <p class="text-[13px] text-(--text-muted)">{empty}</p>
{:else}
  <div class="flex flex-wrap items-center gap-2">
    {#if groups.length}
      <div
        class="inline-flex gap-0.5 rounded-md bg-(--bg) p-0.5"
        role="tablist"
        aria-label="Pick from"
      >
        {#each TABS as t (t.id)}
          <button
            type="button"
            role="tab"
            aria-selected={showing === t.id}
            onclick={() => (tab = t.id)}
            class="rounded px-2.5 py-0.5 text-[13px] font-medium tabular-nums {showing ===
            t.id
              ? 'bg-(--bg-raise-2) text-(--text-primary)'
              : 'text-(--text-muted) hover:text-(--text-primary)'}"
          >
            {t.label} <span class="text-(--text-faint)">{t.count}</span>
          </button>
        {/each}
      </div>
    {/if}
    <SearchBox
      bind:value={query}
      placeholder={groups.length
        ? "Search people and groups"
        : `Search ${plural(friends.length, "friend")}`}
      class="min-w-40 flex-1"
    />
  </div>

  <!-- As tall as the longer tab (36px rows), so switching tabs or searching doesn't move the
       dialog; past 14rem a long list scrolls here instead of stretching it. -->
  <div
    class="mt-2 overflow-y-auto rounded-lg bg-(--bg-raise) p-1"
    style="height: min(14rem, {Math.max(friends.length, groups.length, 2) * 2.25 + 0.5}rem)"
    role="group"
    aria-label={showing === "people" ? "People" : "Groups"}
  >
    {#if showing === "people"}
      {#each shownFriends as f (f.user.id)}
        {@const why = friendTaken(f.user.id)}
        <label
          class="flex items-center gap-2.5 rounded-md px-2 py-1.5 {why
            ? 'opacity-50'
            : 'cursor-pointer hover:bg-(--bg-hover)'}"
        >
          {#if why}
            <input
              type="checkbox"
              checked
              disabled
              class="size-3.75 shrink-0 accent-(--accent)"
            />
          {:else}
            <input
              type="checkbox"
              bind:group={friendIds}
              value={f.user.id}
              class="size-3.75 shrink-0 accent-(--accent)"
            />
          {/if}
          <Head user={f.user} size={24} ring="var(--bg-raise)" />
          <b class="min-w-0 truncate font-medium" title={f.user.displayName}
            >{f.user.displayName}</b
          >
          <small
            class="ml-auto shrink-0 truncate pl-2 text-[13px] text-(--text-faint)"
            >{why ?? `@${f.user.username}`}</small
          >
        </label>
      {:else}
        <p class="px-2 py-6 text-center text-[13px] text-(--text-muted)">
          {query ? `No friends match “${query}”.` : empty}
        </p>
      {/each}
    {:else}
      {#each shownGroups as g (g.id)}
        {@const why = groupTaken(g.id)}
        <label
          class="flex items-center gap-2.5 rounded-md px-2 py-1.5 {why
            ? 'opacity-50'
            : 'cursor-pointer hover:bg-(--bg-hover)'}"
        >
          {#if why}
            <input
              type="checkbox"
              checked
              disabled
              class="size-3.75 shrink-0 accent-(--accent)"
            />
          {:else}
            <input
              type="checkbox"
              bind:group={groupIds}
              value={g.id}
              class="size-3.75 shrink-0 accent-(--accent)"
            />
          {/if}
          <span
            class="grid size-6 shrink-0 place-items-center rounded-md bg-(--bg-raise-2) text-(--text-muted)"
            ><Icon name="group" class="size-3.5" /></span
          >
          <b class="min-w-0 truncate font-medium" title={g.name}>{g.name}</b>
          <small
            class="ml-auto shrink-0 pl-2 text-[13px] text-(--text-faint) tabular-nums"
            >{why ?? plural(g.members.length, "member")}</small
          >
        </label>
      {:else}
        <p class="px-2 py-6 text-center text-[13px] text-(--text-muted)">
          No groups match “{query}”.
        </p>
      {/each}
    {/if}
  </div>

  {#if pickedFriends.length || pickedGroups.length}
    <div class="mt-2 flex items-center gap-2 text-[13px] text-(--text-faint)">
      <span class="tabular-nums">Picked: {pickedSummary}</span>
      <button
        type="button"
        class="rounded px-1.5 text-(--accent) hover:bg-(--bg-hover)"
        onclick={() => {
          friendIds = [];
          groupIds = [];
        }}>Clear</button
      >
    </div>
    <div class="mt-1.5 flex flex-wrap gap-1.5" aria-label="Picked">
      {#each shownGroupChips as g (g.id)}
        <span
          class="flex max-w-56 items-center gap-1.5 rounded-full bg-(--accent)/14 py-0.5 pr-1 pl-2.5 text-[13px] text-(--accent)"
        >
          <Icon name="group" class="size-3" />
          <span class="truncate">{g.name}</span>
          <button
            type="button"
            aria-label="Remove {g.name}"
            onclick={() => (groupIds = groupIds.filter((id) => id !== g.id))}
            class="grid size-4.5 shrink-0 place-items-center rounded-full hover:bg-(--accent)/20"
            ><Icon name="close" class="size-2.5" /></button
          >
        </span>
      {/each}
      {#each shownFriendChips as f (f.user.id)}
        <span
          class="flex max-w-56 items-center gap-1.5 rounded-full bg-(--accent)/14 py-0.5 pr-1 pl-1 text-[13px] text-(--accent)"
        >
          <Head user={f.user} size={18} />
          <span class="truncate">{f.user.displayName}</span>
          <button
            type="button"
            aria-label="Remove {f.user.displayName}"
            onclick={() =>
              (friendIds = friendIds.filter((id) => id !== f.user.id))}
            class="grid size-4.5 shrink-0 place-items-center rounded-full hover:bg-(--accent)/20"
            ><Icon name="close" class="size-2.5" /></button
          >
        </span>
      {/each}
      {#if hiddenChips > 0 || allChips}
        <button
          type="button"
          class="rounded-full px-2.5 py-0.5 text-[13px] text-(--accent) hover:bg-(--bg-hover)"
          onclick={() => (allChips = !allChips)}
          >{allChips ? "Show fewer" : `+${hiddenChips} more`}</button
        >
      {/if}
    </div>
  {/if}
{/if}
