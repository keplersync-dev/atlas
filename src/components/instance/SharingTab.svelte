<script lang="ts">
  import { showError } from "$lib/toasts.svelte";
  import { api } from "$lib/api";
  import { confirmAction } from "$lib/confirm.svelte";
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

  function memberStatus(m: SharedMember) {
    if (m.status === "invited")
      return { text: "Invited", cls: "text-(--text-faint)" };
    if (m.lastSyncedRevision === sync.latestRevision)
      return { text: "Up to date", cls: "text-(--online)" };
    return { text: `On v${m.lastSyncedRevision}`, cls: "text-(--warn)" };
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
      hint: "Can publish changes and invite friends",
    },
    { id: "viewer", label: "Viewer", hint: "Gets updates when you publish" },
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

{#if isOwner}
  <!-- What the two roles mean, next to where they're changed. -->
  <p class="mb-3 text-xs text-(--text-faint)">
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
{:else if shared.error}
  <p class="py-6 text-(--danger)">{shared.error}</p>
{:else if shared.data}
  {#each shared.data.members as m (m.user.id)}
    {@const status = memberStatus(m)}
    <div
      class="grid grid-cols-[32px_minmax(0,1fr)_100px_136px_32px] items-center gap-3.5 rounded-md px-2 py-2 hover:bg-(--bg-hover)"
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
        <div
          role="radiogroup"
          aria-label="Role for {m.user.displayName}"
          class="inline-flex gap-0.5 justify-self-start rounded-md bg-(--bg) p-0.5"
        >
          {#each ROLES as r (r.id)}
            <button
              type="button"
              role="radio"
              aria-checked={m.role === r.id}
              title={r.hint}
              onclick={() => m.role !== r.id && setRole(m, r.id)}
              class="rounded px-2.5 py-0.5 text-[11.5px] font-medium transition-colors
                {m.role === r.id
                ? 'bg-(--bg-raise-2) text-(--text-primary)'
                : 'text-(--text-faint) hover:text-(--text-primary)'}"
            >
              {r.label}
            </button>
          {/each}
        </div>
        <button
          aria-label="Remove {m.user.displayName}"
          onclick={async () => {
            const ok = await confirmAction({
              title: `Remove ${m.user.displayName}?`,
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
        <span class="pl-3 text-[12.5px] text-(--text-muted) capitalize"
          >{m.role}</span
        >
        <span></span>
      {/if}
    </div>
  {/each}
{/if}
