<script lang="ts" module>
  // Module-level state belongs to the file, not to one mounted copy of the page,
  // so the selected tab is still there when you leave and come back.
  let tab = $state<"invites" | "joined" | "yours">("invites");
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import { openDialog } from "$lib/dialogs.svelte";
  import { fileSize, plural, timeAgo, versionLabel } from "$lib/format";
  import { go } from "$lib/nav.svelte";
  import { instances, social } from "$lib/stores";
  import type { SharedInstance } from "$lib/types";
  import BlockArt from "../components/BlockArt.svelte";
  import Button from "../components/Button.svelte";
  import Head from "../components/Head.svelte";
  import Tabs from "../components/Tabs.svelte";

  const joined = $derived(
    social.shared.filter(
      (s) => s.myStatus === "active" && s.myRole !== "owner",
    ),
  );
  const yours = $derived(social.shared.filter((s) => s.myRole === "owner"));

  let busy = $state<string | null>(null);
  let error = $state<string | null>(null);

  async function decline(s: SharedInstance) {
    busy = s.id;
    error = null;
    try {
      await social.declineInvite(s.id);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = null;
    }
  }

  /** The folder on this PC linked to a shared instance, if it's tracked. */
  const localFor = (s: SharedInstance) =>
    s.localInstanceId ? instances.byId(s.localInstanceId) : undefined;
</script>

<!-- One row layout for all three tabs; each tab passes its own middle column and buttons. -->
{#snippet row(
  s: SharedInstance,
  details: string,
  side: Snippet,
  actions: Snippet,
)}
  <div
    class="grid grid-cols-[48px_minmax(0,1fr)_190px_auto] items-center gap-3.5 rounded-md px-2 py-3 hover:bg-(--bg-hover)"
  >
    <BlockArt seed={s.localInstanceId ?? s.id} class="size-12 rounded-md" />
    <div class="min-w-0">
      <b class="block truncate text-[13.5px] font-semibold">{s.name}</b>
      <small class="block truncate text-xs text-(--text-muted)">{details}</small
      >
    </div>
    <div class="min-w-0 text-[12.5px] text-(--text-muted)">
      {@render side()}
    </div>
    <div class="flex justify-end gap-2">{@render actions()}</div>
  </div>
{/snippet}

<div class="mb-6">
  <h1
    class="font-(family-name:--font-display) text-2xl font-semibold tracking-tight"
  >
    Shared
  </h1>
  <p class="mt-1.5 text-(--text-muted)">
    Instances that stay in sync between you and your friends.
  </p>
</div>

{#if !social.signedIn}
  <div class="py-12 text-center text-(--text-muted)">
    <b class="block text-sm text-(--text-primary)">Sign in to share instances</b
    >
    Your tracked instances keep working without an account.
    <div>
      <Button
        variant="primary"
        class="mt-4"
        onclick={() => social.signIn("minecraft")}>Sign in</Button
      >
    </div>
  </div>
{:else}
  <Tabs
    bind:value={tab}
    tabs={[
      { id: "invites", label: "Invites", count: social.invites.length },
      { id: "joined", label: "From friends", count: joined.length },
      { id: "yours", label: "Shared by you", count: yours.length },
    ]}
  />
  {#if error}<p class="mt-2 text-xs text-(--danger)" role="alert">
      {error}
    </p>{/if}

  <div class="mt-2">
    {#if tab === "invites"}
      {#each social.invites as s (s.id)}
        {#snippet from()}
          {@const who = s.invitedBy ?? s.owner}
          <div class="flex items-center gap-2">
            {#if who}<Head user={who} size={20} />{/if}
            <span class="min-w-0">
              <span class="block truncate">{who?.displayName ?? "Someone"}</span
              >
              <span class="block truncate text-[11px] text-(--text-faint)">
                {s.myViaGroup ? `to ${s.myViaGroup.name} · ` : ""}{timeAgo(
                  s.invitedAt,
                )}
              </span>
            </span>
          </div>
        {/snippet}
        {#snippet actions()}
          <Button
            variant="ghost"
            disabled={busy === s.id}
            onclick={() => decline(s)}>Decline</Button
          >
          <Button
            variant="primary"
            onclick={() =>
              openDialog({ kind: "accept", sharedInstanceId: s.id })}
            >Add to a launcher</Button
          >
        {/snippet}
        {@render row(
          s,
          `${versionLabel(s.loader, s.minecraftVersion)} · ${plural(s.contentCount, "item")} · ${fileSize(s.sizeBytes)}${s.description ? ` · ${s.description}` : ""}`,
          from,
          actions,
        )}
      {:else}
        <p class="py-12 text-center text-(--text-muted)">
          No invites. When a friend shares an instance with you or a group
          you're in, it shows up here.
        </p>
      {/each}
    {:else if tab === "joined"}
      {#each joined as s (s.id)}
        {@const local = localFor(s)}
        {#snippet owner()}
          {#if s.owner}
            <div class="flex items-center gap-2">
              <Head user={s.owner} size={20} />{s.owner.displayName}
            </div>
          {:else}
            <span class="text-(--text-faint)">Owner left</span>
          {/if}
        {/snippet}
        {#snippet actions()}
          {#if !local}
            <span class="text-xs text-(--text-faint)"
              >Not tracked on this PC</span
            >
          {:else if local.sync?.state === "update_available" || local.sync?.state === "conflict"}
            <Button
              variant="warn"
              onclick={() => go({ page: "review", id: local.id })}
              >Review update</Button
            >
          {:else if s.frozen}
            <span class="text-xs text-(--text-faint)">Frozen</span>
          {:else}
            <span class="flex items-center gap-1.5 text-xs text-(--text-muted)"
              ><i class="size-1.75 rounded-full bg-(--online)"></i>Up to date</span
            >
          {/if}
          {#if local}<Button
              variant="ghost"
              onclick={() => go({ page: "instance", id: local.id })}
              >Open</Button
            >{/if}
        {/snippet}
        {@render row(
          s,
          `${versionLabel(s.loader, s.minecraftVersion)} · v${s.latestRevision}${local ? ` · ${local.launcher.name}` : ""} · you're ${s.myRole === "editor" ? "an editor" : "a viewer"}`,
          owner,
          actions,
        )}
      {:else}
        <p class="py-12 text-center text-(--text-muted)">
          Nothing from friends yet. Accept an invite and it'll appear here.
        </p>
      {/each}
    {:else}
      {#each yours as s (s.id)}
        {@const local = localFor(s)}
        {@const others = s.members.filter((m) => m.role !== "owner")}
        {#snippet members()}
          <div class="flex items-center gap-2">
            <span class="flex">
              {#each others.slice(0, 4) as m, i (m.user.id)}
                <span class={i ? "-ml-1.5" : ""}
                  ><Head user={m.user} size={20} /></span
                >
              {/each}
            </span>
            <span class="truncate text-(--text-faint)"
              >{plural(others.length, "member")}</span
            >
          </div>
        {/snippet}
        {#snippet actions()}
          {#if local?.sync?.state === "local_changes"}
            <Button
              variant="primary"
              onclick={() =>
                openDialog({ kind: "publish", instanceId: local.id })}
              >Publish</Button
            >
          {/if}
          {#if local}
            <Button
              onclick={() =>
                openDialog({ kind: "invite", instanceId: local.id })}
              >Invite</Button
            >
            <Button
              variant="ghost"
              onclick={() => go({ page: "instance", id: local.id })}
              >Open</Button
            >
          {/if}
        {/snippet}
        {@render row(
          s,
          `${versionLabel(s.loader, s.minecraftVersion)} · v${s.latestRevision}${s.groups.length ? ` · ${s.groups.map((g) => g.group.name).join(", ")}` : ""}`,
          members,
          actions,
        )}
      {:else}
        <p class="py-12 text-center text-(--text-muted)">
          You haven't shared anything yet. Open an instance and choose Share.
        </p>
      {/each}
    {/if}
  </div>
{/if}
