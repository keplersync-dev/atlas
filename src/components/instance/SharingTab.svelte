<script lang="ts">
  import { api } from "$lib/api";
  import { resource } from "$lib/resource.svelte";
  import { social } from "$lib/stores";
  import type { InstanceSyncInfo, MemberRole, SharedMember } from "$lib/types";
  import Head from "../Head.svelte";
  import Icon from "../Icon.svelte";

  let { sync }: { sync: InstanceSyncInfo } = $props();

  // Reads only `sync.sharedInstanceId` before awaiting, so it refetches only when
  // that id changes — not on every progress tick of the instance.
  const shared = resource(() => api.getSharedInstance(sync.sharedInstanceId));

  const isOwner = $derived(sync.role === "owner");
  const myId = $derived(social.session?.user?.id);
  let error = $state<string | null>(null);

  function memberStatus(m: SharedMember) {
    if (m.status === "invited")
      return { text: "Invited", cls: "text-(--text-faint)" };
    if (m.lastSyncedRevision === sync.latestRevision)
      return { text: "Up to date", cls: "text-(--online)" };
    return { text: `On v${m.lastSyncedRevision}`, cls: "text-(--warn)" };
  }

  async function change(action: () => Promise<unknown>) {
    error = null;
    try {
      await action();
      shared.reload();
    } catch (e) {
      error = (e as Error).message;
    }
  }
</script>

<p class="mt-4 mb-2 text-[12.5px] text-(--text-muted)">
  {sync.role === "owner"
    ? "You own this instance."
    : sync.role === "editor"
      ? "You can publish changes and invite friends."
      : "You get updates when the owner publishes."}
  {#if shared.data?.groups.length}
    Shared with {shared.data.groups.map((g) => g.group.name).join(", ")}.
  {/if}
</p>

{#if error}<p class="mb-2 text-xs text-(--danger)" role="alert">{error}</p>{/if}

{#if shared.loading && !shared.data}
  <p class="py-6 text-(--text-muted)">Loading members…</p>
{:else if shared.error}
  <p class="py-6 text-(--danger)">{shared.error}</p>
{:else if shared.data}
  {#each shared.data.members as m (m.user.id)}
    {@const status = memberStatus(m)}
    <div
      class="grid grid-cols-[32px_minmax(0,1fr)_120px_100px_32px] items-center gap-3.5 rounded-md px-2 py-2 hover:bg-(--bg-hover)"
    >
      <Head user={m.user} size={32} />
      <div class="min-w-0">
        <b class="font-medium">{m.user.displayName}</b>
        {#if m.user.id === myId}<span class="text-(--text-faint)">(you)</span
          >{/if}
        <small class="block text-[11.5px] text-(--text-faint)">
          @{m.user.username}{m.viaGroup ? ` · via ${m.viaGroup.name}` : ""}
        </small>
      </div>
      <span class="text-[12px] {status.cls}">{status.text}</span>

      {#if isOwner && m.role !== "owner"}
        <!-- A plain <select>: onchange fires with the new value; we save it, then reload. -->
        <select
          value={m.role}
          onchange={(e) =>
            change(() =>
              api.setSharedMemberRole(
                sync.sharedInstanceId,
                m.user.id,
                e.currentTarget.value as Exclude<MemberRole, "owner">,
              ),
            )}
          aria-label="Role for {m.user.displayName}"
          class="rounded bg-transparent px-1 py-0.5 text-[12.5px] text-(--text-muted) hover:bg-(--bg-hover)"
        >
          <option value="editor" class="bg-(--bg-raise-2)">Editor</option>
          <option value="viewer" class="bg-(--bg-raise-2)">Viewer</option>
        </select>
        <button
          aria-label="Remove {m.user.displayName}"
          onclick={() =>
            change(() =>
              api.removeSharedMember(sync.sharedInstanceId, m.user.id),
            )}
          class="grid size-8 place-items-center rounded-md text-(--text-faint) hover:bg-(--bg-hover) hover:text-(--danger)"
        >
          <Icon name="close" class="size-3.5" />
        </button>
      {:else}
        <span class="text-[12.5px] text-(--text-muted) capitalize"
          >{m.role}</span
        >
        <span></span>
      {/if}
    </div>
  {/each}
{/if}
