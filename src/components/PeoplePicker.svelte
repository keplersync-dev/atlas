<script lang="ts">
  import { fuzzyFilter } from "$lib/fuzzy";
  import { nameList, plural } from "$lib/format";
  import type { Friend, Group, Id } from "$lib/types";
  import GroupPicture from "./GroupPicture.svelte";
  import Head from "./Head.svelte";
  import PersonName from "./PersonName.svelte";
  import SearchBox from "./SearchBox.svelte";

  // Ticks friends (and groups, when given) out of lists that can run to hundreds: one
  // search for both, a tab each, a list that scrolls in place, and the picks on one line
  // under it, with a way to narrow the list to them.
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
  // "Show picked": who was picked when it was clicked, so unticking someone there
  // leaves their row in place to tick again.
  let only = $state<Id[] | null>(null);

  const listFriends = $derived(
    only ? friends.filter((f) => only!.includes(f.user.id)) : friends,
  );
  const listGroups = $derived(
    only ? groups.filter((g) => only!.includes(g.id)) : groups,
  );
  const shownFriends = $derived(
    fuzzyFilter(listFriends, query, (f) => [
      f.user.displayName,
      f.user.username,
    ]),
  );
  const shownGroups = $derived(fuzzyFilter(listGroups, query, (g) => [g.name]));
  // A search (or Show picked) that only finds groups shows them, rather than an empty
  // People tab.
  const narrowed = $derived(!!query || !!only);
  const showing = $derived(
    !groups.length
      ? "people"
      : narrowed && !shownFriends.length && shownGroups.length
        ? "groups"
        : narrowed && !shownGroups.length && shownFriends.length
          ? "people"
          : tab,
  );

  const pickedFriends = $derived(
    friends.filter((f) => friendIds.includes(f.user.id)),
  );
  const pickedGroups = $derived(groups.filter((g) => groupIds.includes(g.id)));

  const picks = $derived(pickedGroups.length + pickedFriends.length);
  const pickedNames = $derived(
    nameList([
      ...pickedGroups.map((g) => g.name),
      ...pickedFriends.map((f) => f.user.displayName),
    ]),
  );
  // A few pictures are enough to recognise; the names say how many.
  const PILE = 5;

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
            class="h-6 rounded px-2.5 text-[13px] font-medium tabular-nums {showing ===
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

  <!-- As tall as the tab's list (36px rows), or the picks under Show picked. A search keeps
       the height, so the dialog doesn't move while typing. Past five rows it scrolls, with
       a sliver of the next row showing so it reads as more. -->
  <div
    class="mt-2 overflow-y-auto rounded-lg bg-(--bg-raise) p-1"
    style="height: min(12.5rem, {Math.max(
      (showing === 'people' ? listFriends : listGroups).length,
      1,
    ) *
      2.25 +
      0.5}rem)"
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
          <span class="min-w-0 flex-1"><PersonName user={f.user} /></span>
          {#if why}<small
              class="shrink-0 truncate pl-2 text-[13px] text-(--text-faint)"
              >{why}</small
            >{/if}
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
          <GroupPicture id={g.id} class="size-6 rounded-md" />
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

  {#if picks || only}
    <div class="mt-2 flex items-center gap-2 text-[13px]">
      <span class="flex shrink-0 gap-0.5" aria-hidden="true">
        {#each pickedGroups.slice(0, PILE) as g (g.id)}
          <GroupPicture id={g.id} class="size-5 rounded-[4px]" />
        {/each}
        {#each pickedFriends.slice(0, Math.max(0, PILE - pickedGroups.length)) as f (f.user.id)}
          <Head user={f.user} size={20} />
        {/each}
      </span>
      <span
        class="min-w-0 flex-1 truncate text-(--text-muted)"
        title={pickedNames}>{picks ? pickedNames : "Nobody picked"}</span
      >
      <button
        type="button"
        aria-pressed={!!only}
        class="h-6 shrink-0 rounded px-1.5 text-(--accent) hover:bg-(--bg-hover)"
        onclick={() => (only = only ? null : [...groupIds, ...friendIds])}
        >{only ? "Show all" : "Show picked"}</button
      >
      <button
        type="button"
        class="h-6 shrink-0 rounded px-1.5 text-(--accent) hover:bg-(--bg-hover)"
        onclick={() => {
          friendIds = [];
          groupIds = [];
          only = null;
        }}>Clear</button
      >
    </div>
  {/if}
{/if}
