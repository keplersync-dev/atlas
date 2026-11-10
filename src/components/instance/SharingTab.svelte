<script lang="ts">
  import { showError } from "$lib/toasts.svelte";
  import { api } from "$lib/api";
  import { confirmAction } from "$lib/confirm.svelte";
  import { fuzzyFilter } from "$lib/fuzzy";
  import { resource } from "$lib/resource.svelte";
  import { social } from "$lib/stores";
  import type {
    Id,
    InstanceSyncInfo,
    MemberRole,
    SharedMember,
  } from "$lib/types";
  import { instances } from "$lib/stores";
  import ConfigFiles from "./ConfigFiles.svelte";
  import Head from "../Head.svelte";
  import PersonName from "../PersonName.svelte";
  import Icon from "../Icon.svelte";
  import SearchBox from "../SearchBox.svelte";
  import Switch from "../Switch.svelte";

  let { instanceId, sync }: { instanceId: Id; sync: InstanceSyncInfo } =
    $props();

  // A search once the list is longer than a screenful.
  let query = $state("");
  const SEARCH_FROM = 8;

  let savingAutoUpdate = $state(false);
  async function setAutoUpdate(on: boolean) {
    savingAutoUpdate = true;
    try {
      await api.updateInstance(instanceId, { autoUpdate: on });
      await instances.load();
    } catch (e) {
      showError((e as Error).message);
    } finally {
      savingAutoUpdate = false;
    }
  }

  // Keyed on the id, not `sync`: that is a new object after every rescan.
  const sharedId = $derived(sync.sharedInstanceId);
  const shared = resource(() => api.getSharedInstance(sharedId));

  const isOwner = $derived(sync.role === "owner");
  // Updates reach the owner's copy only when an editor sends one: with no editors,
  // there's nothing for the owner to install.
  const othersSend = $derived(
    !isOwner ||
      !!shared.data?.members.some(
        (m) => m.role === "editor" && m.status === "active",
      ),
  );
  const myId = $derived(social.session?.user?.id);

  // The owner, then members, then open invites, each by name.
  const ORDER = { active: 1, invited: 2, declined: 3 } as const;
  const members = $derived(
    [...(shared.data?.members ?? [])].sort(
      (a, b) =>
        (a.role === "owner" ? 0 : ORDER[a.status]) -
          (b.role === "owner" ? 0 : ORDER[b.status]) ||
        a.user.displayName.localeCompare(b.user.displayName),
    ),
  );

  function memberStatus(m: SharedMember) {
    if (m.status === "invited")
      return { text: "Invited", cls: "text-(--text-faint)" };
    if (m.lastSyncedRevision === sync.latestRevision)
      return { text: "Up to date", cls: "text-(--online)" };
    return { text: "Not updated yet", cls: "text-(--warn)" };
  }

  async function change(action: () => Promise<unknown>) {
    try {
      await action();
      shared.reload();
    } catch (e) {
      showError((e as Error).message);
    }
  }

  type Role = Exclude<MemberRole, "owner">;
  const ROLES: { id: Role; label: string; hint: string }[] = [
    {
      id: "editor",
      label: "Editor",
      hint: "Can send updates and invite friends",
    },
    { id: "viewer", label: "Viewer", hint: "Gets your updates" },
  ];

  /** Shown at once, then saved; put back if the server says no. */
  function setRole(m: SharedMember, role: Role) {
    const before = m.role;
    m.role = role;
    change(async () => {
      try {
        await api.setSharedMemberRole(sync.sharedInstanceId, m.user.id, role);
      } catch (e) {
        m.role = before;
        throw e;
      }
    });
  }
</script>

<p class="mt-4 mb-2 text-[13px] text-(--text-muted)">
  {sync.role === "owner"
    ? "You share this. Everyone below gets the updates you send."
    : sync.role === "editor"
      ? "You can send updates and invite friends."
      : `${sync.owner?.displayName ?? "The owner"} sends you updates.`}
  {#if shared.data?.groups.length}
    Shared with {shared.data.groups.map((g) => g.group.name).join(", ")}.
  {/if}
</p>

{#if othersSend}
  <div
    class="mb-4 flex items-center gap-3 rounded-lg bg-(--bg-raise) px-3.5 py-3"
  >
    <div class="min-w-0 flex-1">
      <b class="block font-medium"
        >{isOwner
          ? "Install editors' updates automatically"
          : "Keep it up to date automatically"}</b
      >
      <small class="block text-[13px] text-(--text-muted)">
        New versions install before you next play. Atlas still asks when one
        would undo your changes or adds a mod that isn't on Modrinth or
        CurseForge.
      </small>
    </div>
    <Switch
      checked={sync.autoUpdate}
      label={isOwner
        ? "Install editors' updates automatically"
        : "Keep it up to date automatically"}
      disabled={savingAutoUpdate}
      onchange={setAutoUpdate}
    />
  </div>
{/if}

{#if isOwner}
  <!-- What the two roles mean, next to where they're changed. -->
  <p class="mb-3 text-[13px] text-(--text-faint)">
    {#each ROLES as r, i (r.id)}
      <b class="font-semibold text-(--text-muted)">{r.label}</b>: {r.hint.toLowerCase()}{i <
      ROLES.length - 1
        ? " · "
        : "."}
    {/each}
    Change anyone's role at any time.
  </p>
{/if}

{#if shared.loading && !shared.data}
  <p class="py-6 text-(--text-muted)">Loading members…</p>
{:else if !social.signedIn}
  <!-- Not an error: members just need the account. -->
  <p class="py-6 text-(--text-muted)">Sign in to see who has it.</p>
{:else if shared.code === "not_found"}
  <!-- Stopped sharing from another PC, or shared from another account. -->
  <p class="py-6 text-(--text-muted)">
    This isn't shared anymore, or it was shared from another account. Your files
    here are fine.
  </p>
{:else if shared.error}
  <p class="py-6 text-(--danger)">
    Couldn't load who has it: {shared.error}
    <button class="ml-1 underline" onclick={shared.reload}>Try again</button>
  </p>
{:else if shared.data}
  {#if shared.data.members.length >= SEARCH_FROM}
    <SearchBox
      bind:value={query}
      placeholder="Search {shared.data.members.length} members"
      class="mb-2 max-w-72"
    />
  {/if}
  {#each fuzzyFilter( members, query, (m) => [m.user.displayName, m.user.username, m.viaGroup?.name] ) as m (m.user.id)}
    {@const status = memberStatus(m)}
    <div
      class="grid grid-cols-[32px_minmax(0,1fr)_100px_136px_32px] items-center gap-3.5 rounded-md px-2 py-2 hover:bg-(--bg-hover)"
    >
      <Head user={m.user} size={32} />
      <div class="min-w-0">
        <PersonName user={m.user} />
        {#if m.user.id === myId || m.viaGroup}
          <small class="block truncate text-[13px] text-(--text-faint)">
            {[
              m.user.id === myId ? "You" : null,
              m.viaGroup ? `via ${m.viaGroup.name}` : null,
            ]
              .filter(Boolean)
              .join(" · ")}
          </small>
        {/if}
      </div>
      <span class="text-[13px] {status.cls}">{status.text}</span>

      {#if isOwner && m.role !== "owner"}
        <div
          role="radiogroup"
          aria-label="Role for {m.user.displayName} (@{m.user.username})"
          class="inline-flex gap-0.5 justify-self-start rounded-md bg-(--bg) p-0.5"
        >
          {#each ROLES as r (r.id)}
            <button
              type="button"
              role="radio"
              aria-checked={m.role === r.id}
              title={r.hint}
              onclick={() => m.role !== r.id && setRole(m, r.id)}
              class="h-6 rounded px-2.5 text-[13px] font-medium transition-colors
                {m.role === r.id
                ? 'bg-(--bg-raise-2) text-(--text-primary)'
                : 'text-(--text-faint) hover:text-(--text-primary)'}"
            >
              {r.label}
            </button>
          {/each}
        </div>
        <button
          aria-label="Remove {m.user.displayName} (@{m.user.username})"
          onclick={async () => {
            const ok = await confirmAction({
              title: `Remove ${m.user.displayName} (@${m.user.username})?`,
              message:
                "They stop getting updates. Their copy stays on their PC as a normal instance.",
              confirmLabel: "Remove",
              danger: true,
            });
            if (ok)
              change(() =>
                api.removeSharedMember(sync.sharedInstanceId, m.user.id),
              );
          }}
          class="grid size-8 place-items-center rounded-md text-(--text-faint) hover:bg-(--bg-hover) hover:text-(--danger)"
        >
          <Icon name="close" class="size-3.5" />
        </button>
      {:else}
        <!-- pl-3: lines up with the first label of the role switch above or below. -->
        <span class="pl-3 text-[13px] text-(--text-muted) capitalize"
          >{m.role}</span
        >
        <span></span>
      {/if}
    </div>
  {:else}
    <p class="px-2 py-6 text-[13px] text-(--text-muted)">
      No members match “{query}”.
    </p>
  {/each}
{/if}

{#if sync.role !== "viewer"}
  <ConfigFiles {instanceId} />
{/if}
