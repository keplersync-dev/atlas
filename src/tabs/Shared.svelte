<script lang="ts" module>
  // Search and filter survive leaving the page, like the friends' dropdowns.
  let query = $state("");
  let filter = $state<"all" | "needs" | "local" | "not_added">("all");
</script>

<script lang="ts">
  import { api } from "$lib/api";
  import { plural } from "$lib/format";
  import { social } from "$lib/stores";
  import type { SharedInstance } from "$lib/types";
  import AttentionBadges from "../components/AttentionBadges.svelte";
  import Button from "../components/Button.svelte";
  import Icon from "../components/Icon.svelte";
  import SharedRow, {
    byAttention,
    localFor,
    rowState,
  } from "../components/SharedRow.svelte";
  import ActivityFeed, {
    SHARING_TYPES,
  } from "../components/shared/ActivityFeed.svelte";
  import ShareButton from "../components/shared/ShareButton.svelte";
  import SignInButtons from "../components/SignInButtons.svelte";
  import StorageMeter from "../components/shared/StorageMeter.svelte";

  // Every shared instance, split by direction. To see one friend's instances,
  // open them on the Friends page.
  const all = $derived(social.shared.filter((s) => s.myStatus !== "declined"));

  const FILTERS = [
    { key: "all", label: "All", test: () => true },
    { key: "needs", label: "Needs you", test: (s) => !!rowState(s).attention },
    { key: "local", label: "On this PC", test: (s) => !!localFor(s) },
    {
      key: "not_added",
      label: "Not added",
      test: (s) => !localFor(s),
    },
  ] as const satisfies readonly {
    key: typeof filter;
    label: string;
    test: (s: SharedInstance) => boolean;
  }[];

  const matchesQuery = (s: SharedInstance) => {
    const q = query.trim().toLowerCase();
    if (!q) return true;
    return [s.name, s.owner?.displayName, s.description, s.minecraftVersion]
      .filter(Boolean)
      .some((t) => t!.toLowerCase().includes(q));
  };
  const shown = $derived(
    all.filter(
      (s) => matchesQuery(s) && FILTERS.find((f) => f.key === filter)!.test(s),
    ),
  );
  const narrowed = $derived(filter !== "all" || query.trim() !== "");
  // With a filter or search on, only the lists with matches show; this is said instead
  // when neither has any.
  const noMatches = $derived(
    query.trim()
      ? `Nothing matches “${query.trim()}”.`
      : {
          all: "",
          needs: "Nothing needs you right now.",
          local: "None of your shared instances are on this PC.",
          not_added: "Every shared instance is on this PC.",
        }[filter],
  );

  const mine = $derived(
    shown.filter((s) => s.myRole === "owner").toSorted(byAttention),
  );
  const withMe = $derived(
    shown.filter((s) => s.myRole !== "owner").toSorted(byAttention),
  );

  // ── Update all: shared instances on this PC with an update and nothing to decide ──
  const updatable = $derived(
    all.flatMap((s) => {
      const local = localFor(s);
      return local?.sync?.state === "update_available" && !local.isRunning
        ? [{ shared: s, local }]
        : [];
    }),
  );
  let updating = $state(false);
  let updateNote = $state<{ text: string; ok: boolean } | null>(null);

  // Updates that ask nothing of the user go ahead; any with a question (config files
  // you changed, manual downloads, a launcher change) are left for their own Review page.
  async function updateAll() {
    updating = true;
    updateNote = null;
    const asks: string[] = [];
    let done = 0;
    let failed = 0;
    for (const { shared, local } of updatable) {
      try {
        const p = await api.previewSync(local.id);
        const needsYou =
          p.configConflicts.length > 0 ||
          p.manualDownloads.length > 0 ||
          p.overwrittenLocalChanges.length > 0 ||
          (!!p.launcherChange && !p.launcherChange.automatic);
        if (needsYou) {
          asks.push(shared.name);
          continue;
        }
        await api.syncInstance(local.id);
        done++;
      } catch {
        failed++;
      }
    }
    updating = false;
    const parts = [
      done && `Updating ${plural(done, "instance")}`,
      asks.length &&
        `${asks.join(", ")} need${asks.length === 1 ? "s" : ""} a look first`,
      failed && `${plural(failed, "update")} didn't start`,
    ].filter(Boolean);
    updateNote = { text: parts.join(" · "), ok: !failed };
  }

  const activity = $derived(
    social.activity.filter((a) => SHARING_TYPES.has(a.type)),
  );

  type Section = {
    key: string;
    title: string;
    blurb: string;
    icon: "share" | "download";
    items: SharedInstance[];
    empty: string;
    showOwner: boolean;
  };
  const sections = $derived<Section[]>([
    {
      key: "mine",
      title: "Shared by you",
      blurb: "Instances you own. Friends get your changes when you publish.",
      icon: "share",
      items: mine,
      empty:
        "You haven't shared anything yet. Open an instance and choose Share.",
      showOwner: false,
    },
    {
      key: "with-me",
      title: "Shared with you",
      blurb: "From friends and groups. You get their updates.",
      icon: "download",
      items: withMe,
      empty:
        "When a friend shares an instance with you or a group you're in, it shows up here.",
      showOwner: true,
    },
  ]);
</script>

<!-- Snippets keep the markup below short: tools, then the two lists. -->

{#snippet search()}
  <label class="relative block max-w-72 min-w-36 flex-1">
    <span
      class="pointer-events-none absolute top-1/2 left-2.5 flex -translate-y-1/2 text-(--text-faint)"
    >
      <Icon name="search" class="size-3.5" />
    </span>
    <input
      bind:value={query}
      placeholder="Search shared instances"
      class="h-8 w-full rounded-md border border-(--line-2) bg-(--bg-raise) pr-2.5 pl-8 text-[12.5px] placeholder:text-(--text-faint) focus:border-(--accent) focus:outline-none"
    />
  </label>
{/snippet}

{#snippet chips()}
  <div class="flex flex-wrap gap-1.5" role="radiogroup" aria-label="Filter">
    {#each FILTERS as f (f.key)}
      {@const n = all.filter(f.test).length}
      <button
        role="radio"
        aria-checked={filter === f.key}
        onclick={() => (filter = f.key)}
        class="h-7 rounded-full px-3 text-xs {filter === f.key
          ? 'bg-(--accent)/15 font-semibold text-(--accent)'
          : 'text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)'}"
      >
        {f.label}
        {#if f.key !== "all"}<span class="ml-0.5 opacity-70">{n}</span>{/if}
      </button>
    {/each}
  </div>
{/snippet}

{#snippet cards()}
  <div class="grid grid-cols-1 items-start gap-10">
    {#if narrowed && sections.every((sec) => !sec.items.length)}
      <p
        class="rounded-xl bg-(--bg-raise) px-4 py-6 text-[12.5px] text-(--text-muted)"
      >
        {noMatches}
      </p>
    {/if}
    {#each sections.filter((sec) => !narrowed || sec.items.length) as sec (sec.key)}
      <section class="overflow-hidden rounded-xl bg-(--bg-raise)">
        <header
          class="flex items-center gap-3 border-b border-(--line) px-4 py-3.5"
        >
          <span
            class="grid size-8 place-items-center rounded-lg bg-(--bg-raise-2) text-(--text-muted)"
          >
            <Icon name={sec.icon} class="size-4" />
          </span>
          <span class="min-w-0 flex-1">
            <b class="font-semibold">{sec.title}</b>
            <span class="ml-1 text-(--text-faint)">{sec.items.length}</span>
            <small class="block truncate text-xs text-(--text-faint)"
              >{sec.blurb}</small
            >
          </span>
          <AttentionBadges items={sec.items} />
        </header>
        {#each sec.items as s (s.id)}
          <SharedRow {s} showOwner={sec.showOwner} />
        {:else}
          <p class="px-4 py-6 text-[12.5px] text-(--text-muted)">{sec.empty}</p>
        {/each}
      </section>
    {/each}
  </div>
{/snippet}

{#if !social.signedIn}
  <div class="py-12 text-center text-(--text-muted)">
    <b class="block text-sm text-(--text-primary)">Sign in to share instances</b
    >
    Your tracked instances keep working without an account.
    <SignInButtons />
  </div>
{:else}
  <!-- Lists on the left; activity and storage in a column on the right (like Friends). -->
  <div
    class="grid grid-cols-[minmax(0,1fr)_320px] items-start gap-8 max-[1150px]:grid-cols-1"
  >
    <div>
      <div class="mb-5 flex flex-wrap items-center gap-x-4 gap-y-3">
        {@render search()}
        {@render chips()}
        <ShareButton class="ml-auto" />
      </div>
      {#if updatable.length > 1 || updating || updateNote}
        <div
          class="mb-5 flex items-center gap-3 rounded-xl bg-(--warn)/10 px-4 py-2.5"
        >
          <span class="size-2 shrink-0 rounded-full bg-(--warn)"></span>
          <span class="min-w-0 flex-1 truncate">
            {#if updateNote}
              <span class={updateNote.ok ? "" : "text-(--danger)"}
                >{updateNote.text}</span
              >
            {:else}
              <b class="font-semibold"
                >{plural(updatable.length, "update")} ready</b
              >
              <span class="text-(--text-muted)">
                · {updatable.map((u) => u.shared.name).join(", ")}</span
              >
            {/if}
          </span>
          {#if updateNote}
            <Button
              size="sm"
              variant="ghost"
              onclick={() => (updateNote = null)}>Dismiss</Button
            >
          {:else}
            <Button
              size="sm"
              variant="warn"
              disabled={updating}
              onclick={updateAll}
              >{updating ? "Updating…" : "Update all"}</Button
            >
          {/if}
        </div>
      {/if}
      {@render cards()}
    </div>
    <aside class="flex flex-col gap-4">
      <section class="rounded-xl bg-(--bg-raise) p-4">
        <h2 class="mb-1.5 font-semibold">Recent activity</h2>
        <ActivityFeed items={activity.slice(0, 7)} />
      </section>
      <section class="rounded-xl bg-(--bg-raise) p-4">
        <StorageMeter />
      </section>
    </aside>
  </div>
{/if}
