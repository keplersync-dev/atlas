<script lang="ts" module>
  // Search, filter and sort survive leaving the page, like the other pages' tools.
  let query = $state("");
  let filter = $state<"all" | "needs" | "shared" | "local">("all");
  let sort = $state<"played" | "name" | "added">("played");
</script>

<script lang="ts">
  import { fuzzy, fuzzyFilter } from "$lib/fuzzy";
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

  const STEPS = [
    {
      icon: "plus",
      title: "Add your instance",
      text: "From Prism, the Modrinth App or CurseForge. Atlas only reads it.",
    },
    {
      icon: "share",
      title: "Share it with friends",
      text: "They pick their launcher, and Atlas sets it up for them.",
    },
    {
      icon: "refresh",
      title: "Stay the same",
      text: "When you change a mod, their copy follows.",
    },
  ];
  const LAUNCHERS = [
    "Prism Launcher",
    "Modrinth App",
    "CurseForge",
    "Minecraft Launcher",
  ];

  const matches = (name: string) => fuzzy(query, name) !== null;

  // Attention first (updates, changes to share, problems), then the chosen order.
  const ORDER = {
    played: (a: InstanceSummary, b: InstanceSummary) =>
      (b.lastPlayedAt ?? "").localeCompare(a.lastPlayedAt ?? ""),
    name: (a: InstanceSummary, b: InstanceSummary) =>
      a.name.localeCompare(b.name),
    added: (a: InstanceSummary, b: InstanceSummary) =>
      b.trackedAt.localeCompare(a.trackedAt),
  };
  const shown = $derived.by(() => {
    const kept = instances.list.filter(
      (i) => matches(i.name) && FILTERS.find((f) => f.key === filter)!.test(i),
    );
    // Searching: best match first. Otherwise attention first, then the chosen order.
    return query.trim()
      ? fuzzyFilter(kept, query, (i) => [i.name])
      : kept.toSorted(
          (a, b) =>
            Number(needsAttention(b)) - Number(needsAttention(a)) ||
            ORDER[sort](a, b),
        );
  });
  // Under All, cards that need the player get their own section above the rest.
  const attention = $derived(
    filter === "all" ? shown.filter(needsAttention) : [],
  );
  const rest = $derived(
    filter === "all" ? shown.filter((i) => !needsAttention(i)) : shown,
  );
  // Invites only show under All; the search narrows them too.
  const invites = $derived(
    filter === "all" ? social.invites.filter((s) => matches(s.name)) : [],
  );
</script>

<!-- A section heading with a coloured bar; amber for what needs the player. -->
{#snippet heading(
  label: string,
  count: number,
  tone: "accent" | "warn" = "accent",
)}
  <h2
    class="mb-3 flex items-center gap-2 font-(family-name:--font-display) text-[16px] font-semibold"
  >
    <i
      class="h-4 w-1 rounded-full {tone === 'warn'
        ? 'bg-(--warn)'
        : 'bg-(--accent)'}"
      aria-hidden="true"
    ></i>
    {label}
    <span class="font-normal text-(--text-faint)">{count}</span>
  </h2>
{/snippet}

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
      <!-- A new friend's first visit: the invite that brought them comes first. -->
      {#if social.invites.length}
        <section class="mb-4">
          {@render heading("Shared with you", social.invites.length)}
          <div
            class="grid grid-cols-[repeat(auto-fill,minmax(320px,1fr))] gap-3"
          >
            {#each social.invites as invite (invite.id)}
              <InviteCard {invite} />
            {/each}
          </div>
        </section>
      {/if}
      <!-- Centred on big windows rather than stuck to the top left. -->
      <div class="mx-auto max-w-3xl pt-[max(2.5rem,14vh)]">
        <h1
          class="font-(family-name:--font-display) text-3xl font-semibold tracking-tight"
        >
          Keep your friends on the same mods.
        </h1>
        <ol
          class="mt-7 grid grid-cols-[repeat(auto-fit,minmax(180px,1fr))] gap-3"
        >
          {#each STEPS as step, i (step.title)}
            <li
              class="rounded-xl border border-(--line) bg-(--bg-raise) px-4 py-3.5 shadow-[0_8px_24px_#0000004d]"
            >
              <span
                class="grid size-8 place-items-center rounded-full bg-(--accent)/14 text-(--accent)"
              >
                <Icon name={step.icon} class="size-4" />
              </span>
              <b class="mt-3 block text-[15px] font-semibold"
                >{i + 1}. {step.title}</b
              >
              <p class="mt-0.5 text-[13px] text-(--text-muted)">{step.text}</p>
            </li>
          {/each}
        </ol>
        <p
          class="mt-5 flex flex-wrap items-center gap-1.5 text-[13px] text-(--text-muted)"
        >
          Works with
          {#each LAUNCHERS as name (name)}
            <span
              class="rounded-full border border-(--line) bg-(--bg-raise) px-2.5 py-0.5"
              >{name}</span
            >
          {/each}
        </p>
        <Button
          variant="primary"
          size="lg"
          class="mt-7"
          onclick={() => openDialog({ kind: "track" })}
        >
          Choose instances
        </Button>
        <!-- The other way in: a friend sent them here. -->
        <p
          class="mt-8 flex items-start gap-2.5 border-t border-(--line) pt-5 text-[13px] text-(--text-muted)"
        >
          <Icon
            name="inbox"
            class="mt-0.5 size-4 shrink-0 text-(--text-faint)"
          />
          {social.signedIn
            ? "Joining a friend? When they share a pack with you, it shows up here."
            : "Joining a friend? Sign in on the left, and the pack they shared shows up here."}
        </p>
      </div>
    {:else}
      <!-- Search and actions on one row, filters under them: the same at every width. -->
      <div class="mb-3 flex items-center gap-2">
        <label class="relative block max-w-72 min-w-36 flex-1">
          <span
            class="pointer-events-none absolute top-1/2 left-2.5 flex -translate-y-1/2 text-(--text-faint)"
          >
            <Icon name="search" class="size-3.5" />
          </span>
          <input
            bind:value={query}
            placeholder="Search instances"
            class="h-8 w-full rounded-md border border-(--line-2) bg-(--bg-raise) pr-2.5 pl-8 text-[13px] placeholder:text-(--text-faint) focus:border-(--accent) focus:outline-none"
          />
        </label>
        <div class="ml-auto flex shrink-0 items-center gap-2">
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
            icon="plus"
            onclick={() => openDialog({ kind: "track" })}
          >
            Add instances
          </Button>
        </div>
      </div>
      <div class="mb-7">
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
              class="h-7 rounded-full px-3 text-[13px] {filter === f.key
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
      </div>

      <!-- Under All, what needs the player gets its own section first. -->
      {#if attention.length}
        <section class="mb-9">
          {@render heading("Needs you", attention.length, "warn")}
          <div
            class="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-4"
          >
            {#each attention as inst (inst.id)}
              <InstanceCard instance={inst} />
            {/each}
          </div>
        </section>
      {/if}

      <!-- Invites are instances too, so they get a row of cards above yours. -->
      {#if invites.length}
        <section class="mb-9">
          {@render heading("Shared with you", invites.length)}
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
        <!-- Only needed to tell them apart from the sections above. -->
        {#if (invites.length || attention.length) && rest.length}
          {@render heading("Your instances", rest.length)}
        {/if}
        <!-- auto-fill: as many 240px+ columns as fit, so wide windows get more columns -->
        <div class="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-4">
          {#each rest as inst (inst.id)}
            <InstanceCard instance={inst} />
          {:else}
            {#if !attention.length}
              <p class="col-span-full py-10 text-center text-(--text-muted)">
                {query.trim()
                  ? `No instances match “${query.trim()}”.`
                  : {
                      all: "",
                      needs: "Nothing needs you right now.",
                      shared: "None of your instances are shared yet.",
                      local: "Every instance you've added is shared.",
                    }[filter]}
              </p>
            {/if}
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
