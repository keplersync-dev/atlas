<script lang="ts">
  import { instances, social } from "$lib/stores";
  import { openDialog } from "$lib/dialogs.svelte";
  import Button from "../components/Button.svelte";
  import FriendsRail from "../components/FriendsRail.svelte";
  import InstanceCard, {
    needsAttention,
  } from "../components/InstanceCard.svelte";
  import InviteCard from "../components/InviteCard.svelte";

  // Attention first (updates, changes to share, problems), then most recently played.
  const sorted = $derived(
    instances.list.toSorted((a, b) => {
      const byAttention = Number(needsAttention(b)) - Number(needsAttention(a));
      if (byAttention) return byAttention;
      return (b.lastPlayedAt ?? "").localeCompare(a.lastPlayedAt ?? "");
    }),
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
      <!-- Invites are instances too, so they get a row of cards above yours. -->
      {#if social.invites.length}
        <section class="mb-9">
          <h2 class="mb-3 text-[15px] font-semibold">
            Shared with you <span class="font-normal text-(--text-faint)"
              >{social.invites.length}</span
            >
          </h2>
          <div
            class="grid grid-cols-[repeat(auto-fill,minmax(320px,1fr))] gap-3"
          >
            {#each social.invites as invite (invite.id)}
              <InviteCard {invite} />
            {/each}
          </div>
        </section>
      {/if}

      <section>
        <div class="mb-3 flex items-baseline justify-between">
          <h2 class="text-[15px] font-semibold">
            Your instances <span class="font-normal text-(--text-faint)"
              >{instances.list.length}</span
            >
          </h2>
          <button
            class="text-xs text-(--accent) hover:underline"
            onclick={() => openDialog({ kind: "track" })}
          >
            Track another instance
          </button>
        </div>
        <!-- auto-fill: as many 240px+ columns as fit, so wide windows get more columns -->
        <div class="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-4">
          {#each sorted as inst (inst.id)}
            <InstanceCard instance={inst} />
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
