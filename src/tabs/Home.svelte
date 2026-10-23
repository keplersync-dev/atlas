<script lang="ts" module>
  // Search, filter and sort survive leaving the page, like the other pages' tools.
  let query = $state("");
  let filter = $state<"all" | "needs" | "shared" | "local">("all");
  let sort = $state<"played" | "name" | "added">("played");
</script>

<script lang="ts">
  import type { InstanceSummary } from "$lib/types";
  import { instances, social } from "$lib/stores";
  import { openDialog } from "$lib/dialogs.svelte";
  import Button from "../components/Button.svelte";
  import FriendsRail from "../components/FriendsRail.svelte";
  import Icon from "../components/Icon.svelte";
  import InstanceCard, {
    needsAttention,
  } from "../components/InstanceCard.svelte";
  import InviteCard from "../components/InviteCard.svelte";
  import Select from "../components/Select.svelte";

  const FILTERS = [
    { key: "all", label: "All", test: () => true },
    { key: "needs", label: "Needs you", test: needsAttention },
    { key: "shared", label: "Shared", test: (i) => !!i.sync },
    { key: "local", label: "Not shared", test: (i) => !i.sync },
  ] as const satisfies readonly {
    key: typeof filter;
    label: string;
    test: (i: InstanceSummary) => boolean;
  }[];

  const matches = (name: string) =>
    name.toLowerCase().includes(query.trim().toLowerCase());

  // Attention first (updates, changes to share, problems), then the chosen order.
  const ORDER = {
    played: (a: InstanceSummary, b: InstanceSummary) =>
      (b.lastPlayedAt ?? "").localeCompare(a.lastPlayedAt ?? ""),
    name: (a: InstanceSummary, b: InstanceSummary) =>
      a.name.localeCompare(b.name),
    added: (a: InstanceSummary, b: InstanceSummary) =>
      b.trackedAt.localeCompare(a.trackedAt),
  };
  const shown = $derived(
    instances.list
      .filter(
        (i) =>
          matches(i.name) && FILTERS.find((f) => f.key === filter)!.test(i),
      )
      .toSorted(
        (a, b) =>
          Number(needsAttention(b)) - Number(needsAttention(a)) ||
          ORDER[sort](a, b),
      ),
  );
  // Invites only show under All; the search narrows them too.
  const invites = $derived(
    filter === "all" ? social.invites.filter((s) => matches(s.name)) : [],
  );
</script>

<div class="flex min-h-full">
  <!-- ── Main column: grows with the window ── -->
  <div class="min-w-0 flex-1 px-9 pt-7 pb-16">
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
      <div class="max-w-xl pt-10">
        <h1
          class="font-(family-name:--font-display) text-3xl font-semibold tracking-tight"
        >
          Keep your friends on the same mods.
        </h1>
        <p class="mt-2.5 text-sm text-(--text-muted)">
          Atlas works alongside the launcher you already use. Track an instance
          from Prism, the Modrinth App or CurseForge to share it or receive
          updates for it.
        </p>
        <Button
          variant="primary"
          size="lg"
          class="mt-6"
          onclick={() => openDialog({ kind: "track" })}
        >
          Choose instances
        </Button>
      </div>
    {:else}
      <div class="mb-7 flex flex-wrap items-center gap-x-3 gap-y-3">
        <!-- The search box gives up width first, so the row fits beside the friends column. -->
        <label class="relative block max-w-64 min-w-36 flex-1">
          <span
            class="pointer-events-none absolute top-1/2 left-2.5 flex -translate-y-1/2 text-(--text-faint)"
          >
            <Icon name="search" class="size-3.5" />
          </span>
          <input
            bind:value={query}
            placeholder="Search instances"
            class="h-8 w-full rounded-md border border-(--line-2) bg-(--bg-raise) pr-2.5 pl-8 text-[12.5px] placeholder:text-(--text-faint) focus:border-(--accent) focus:outline-none"
          />
        </label>
        <div
          class="flex flex-wrap gap-1.5"
          role="radiogroup"
          aria-label="Filter"
        >
          {#each FILTERS as f (f.key)}
            <button
              role="radio"
              aria-checked={filter === f.key}
              onclick={() => (filter = f.key)}
              class="h-7 rounded-full px-3 text-xs {filter === f.key
                ? 'bg-(--accent)/15 font-semibold text-(--accent)'
                : 'text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)'}"
            >
              {f.label}
              {#if f.key !== "all"}<span class="ml-0.5 opacity-70"
                  >{instances.list.filter(f.test).length}</span
                >{/if}
            </button>
          {/each}
        </div>
        <div class="ml-auto flex items-center gap-2">
          <Select
            bind:value={sort}
            label="Sort"
            options={[
              { value: "played", label: "Last played" },
              { value: "name", label: "Name" },
              { value: "added", label: "Recently added" },
            ]}
          />
          <Button
            variant="primary"
            class="gap-1.5"
            onclick={() => openDialog({ kind: "track" })}
          >
            <Icon name="plus" class="size-3.5" />Track instances
          </Button>
        </div>
      </div>

      <!-- Invites are instances too, so they get a row of cards above yours. -->
      {#if invites.length}
        <section class="mb-9">
          <h2 class="mb-3 text-[15px] font-semibold">
            Shared with you <span class="font-normal text-(--text-faint)"
              >{invites.length}</span
            >
          </h2>
          <div
            class="grid grid-cols-[repeat(auto-fill,minmax(320px,1fr))] gap-3"
          >
            {#each invites as invite (invite.id)}
              <InviteCard {invite} />
            {/each}
          </div>
        </section>
      {/if}

      <section>
        <!-- Only needed to tell them apart from the invites above. -->
        {#if invites.length}
          <h2 class="mb-3 text-[15px] font-semibold">
            Your instances <span class="font-normal text-(--text-faint)"
              >{shown.length}</span
            >
          </h2>
        {/if}
        <!-- auto-fill: as many 240px+ columns as fit, so wide windows get more columns -->
        <div class="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-4">
          {#each shown as inst (inst.id)}
            <InstanceCard instance={inst} />
          {:else}
            <p class="col-span-full py-10 text-center text-(--text-muted)">
              {query.trim()
                ? `No instances match “${query.trim()}”.`
                : {
                    all: "",
                    needs: "Nothing needs you right now.",
                    shared: "None of your instances are shared yet.",
                    local: "Every instance you track is shared.",
                  }[filter]}
            </p>
          {/each}
        </div>
      </section>
    {/if}
  </div>

  <!-- ── Friends column: pinned to the right edge, stays put while the main column scrolls ── -->
  <aside
    class="sticky top-0 h-[calc(100vh-36px)] w-72 shrink-0 overflow-y-auto border-l border-white/4 bg-(--bg-side)/70 px-5 pt-7 pb-6 max-[1180px]:hidden"
  >
    <FriendsRail />
  </aside>
</div>
