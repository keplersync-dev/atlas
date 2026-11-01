<script lang="ts">
  import { api } from "$lib/api";
  import { copier } from "$lib/clipboard.svelte";
  import { confirmAction } from "$lib/confirm.svelte";
  import { plural } from "$lib/format";
  import { social } from "$lib/stores";
  import type { Group } from "$lib/types";
  import Button from "../Button.svelte";
  import Head from "../Head.svelte";
  import Icon from "../Icon.svelte";

  const clip = copier(2200);

  // Which small inline form is open. One variable instead of two booleans:
  // opening one closes the other automatically.
  let form = $state<"none" | "create" | "join">("none");
  let name = $state("");
  let joinCode = $state("");
  let busy = $state(false);
  let message = $state<{ text: string; ok: boolean } | null>(null);
  // The group whose "Invite friends" picker is open, and who's ticked in it.
  let inviting = $state<string | null>(null);
  let picked = $state<string[]>([]);
  const invitable = (g: Group) =>
    social.friends.filter(
      (f) => !g.members.some((m) => m.user.id === f.user.id),
    );

  function openInvite(g: Group) {
    inviting = inviting === g.id ? null : g.id;
    picked = [];
    message = null;
  }

  function sendInvites(g: Group) {
    run(async () => {
      await api.inviteFriendsToGroup(g.id, picked);
      const n = picked.length;
      inviting = null;
      picked = [];
      return `Invited ${plural(n, "friend")} to ${g.name}. They'll see it on their Friends page.`;
    });
  }

  async function run(action: () => Promise<string>) {
    busy = true;
    message = null;
    try {
      message = { text: await action(), ok: true };
      form = "none";
    } catch (e) {
      message = { text: (e as Error).message, ok: false };
    } finally {
      busy = false;
    }
  }

  function create(e: SubmitEvent) {
    e.preventDefault();
    run(async () => {
      const g = await social.createGroup(name.trim());
      name = "";
      // Straight on to the next step: who's in it.
      if (social.friends.length) inviting = g.id;
      return `Created ${g.name}. Pick friends to invite, or copy a link.`;
    });
  }

  function join(e: SubmitEvent) {
    e.preventDefault();
    run(async () => {
      const g = await social.joinGroup(joinCode);
      joinCode = "";
      return `You joined ${g.name}.`;
    });
  }

  function copyInvite(g: Group) {
    run(async () => {
      const invite = await api.createGroupInvite(g.id, {
        maxUses: null,
        expiresInHours: 24 * 7,
      });
      const ok = await clip.copy(invite.link, g.id);
      return ok
        ? `Invite link for ${g.name} copied. It works for 7 days.`
        : `Couldn't copy automatically. Invite link for ${g.name}: ${invite.link}`;
    });
  }

  async function leave(g: Group) {
    const ok = await confirmAction({
      title: `Leave ${g.name}?`,
      message: "Instances shared with the group stay on your PC.",
      confirmLabel: "Leave group",
      danger: true,
    });
    if (!ok) return;
    run(async () => {
      await social.leaveGroup(g.id);
      return `You left ${g.name}.`;
    });
  }
</script>

<div class="rounded-xl border border-(--line) bg-(--bg-raise) p-4.5">
  <div class="flex items-center justify-between">
    <h2 class="font-semibold">Groups</h2>
    <div class="flex gap-1.5">
      <Button
        size="sm"
        aria-pressed={form === "join"}
        class={form === "join" ? "border-(--accent)!" : ""}
        onclick={() => (form = form === "join" ? "none" : "join")}
      >
        <Icon name="login" class="size-3.5" />
        Join
      </Button>
      <Button
        size="sm"
        aria-pressed={form === "create"}
        class={form === "create" ? "border-(--accent)!" : ""}
        onclick={() => (form = form === "create" ? "none" : "create")}
      >
        <Icon name="plus" class="size-3.5" />
        New group
      </Button>
    </div>
  </div>
  <p class="mt-1 mb-1.5 text-[13px] text-(--text-muted)">
    Share an instance with a whole group at once. New members get it
    automatically.
  </p>

  {#if form === "create"}
    <form onsubmit={create} class="my-2 flex gap-2">
      <input
        bind:value={name}
        maxlength="32"
        placeholder="Group name"
        class="h-8 min-w-0 flex-1 rounded-md border border-(--line-2) bg-(--bg) px-3 outline-none focus:border-(--accent)"
      />
      <Button
        type="submit"
        size="sm"
        variant="primary"
        disabled={busy || !name.trim()}>Create</Button
      >
    </form>
  {:else if form === "join"}
    <form onsubmit={join} class="my-2 flex gap-2">
      <input
        bind:value={joinCode}
        placeholder="Invite code or link"
        class="h-8 min-w-0 flex-1 rounded-md border border-(--line-2) bg-(--bg) px-3 font-mono text-[13px] outline-none focus:border-(--accent)"
      />
      <Button
        type="submit"
        size="sm"
        variant="primary"
        disabled={busy || !joinCode.trim()}>Join</Button
      >
    </form>
  {/if}

  {#if message}
    <p
      class="my-1.5 text-[13px] {message.ok
        ? 'text-(--online)'
        : 'text-(--danger)'}"
      role="status"
    >
      {message.text}
    </p>
  {/if}

  <div class="divide-y divide-(--line)">
    {#each social.groups as g (g.id)}
      <div class="flex items-center gap-3 py-2">
        <span class="flex">
          {#each g.members.slice(0, 4) as m, i (m.user.id)}
            <span class={i ? "-ml-1.5" : ""}
              ><Head user={m.user} size={22} /></span
            >
          {/each}
        </span>
        <div class="min-w-0 flex-1">
          <b class="block truncate font-medium">{g.name}</b>
          <small class="text-[13px] text-(--text-faint) capitalize">
            {plural(g.members.length, "member")} · {plural(
              g.sharedInstanceIds.length,
              "instance",
            )} · {g.myRole}
          </small>
        </div>
        {#if g.myRole === "member"}
          <Button
            size="sm"
            variant="ghost"
            disabled={busy}
            onclick={() => leave(g)}>Leave</Button
          >
        {:else}
          <Button
            size="sm"
            variant="ghost"
            disabled={busy}
            title="For people who aren't your friends yet"
            onclick={() => copyInvite(g)}
          >
            {clip.copied === g.id ? "Copied" : "Copy link"}
          </Button>
          <Button
            size="sm"
            aria-expanded={inviting === g.id}
            disabled={busy}
            onclick={() => openInvite(g)}
          >
            <Icon name="users" class="size-3.5" />Invite friends
          </Button>
        {/if}
      </div>
      {#if inviting === g.id}
        {@const people = invitable(g)}
        <div class="mb-2 rounded-lg bg-(--bg) p-3">
          {#if people.length}
            <div class="grid gap-0.5">
              {#each people as f (f.user.id)}
                <label
                  class="flex cursor-pointer items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-(--bg-hover)"
                >
                  <input
                    type="checkbox"
                    bind:group={picked}
                    value={f.user.id}
                    class="size-4 accent-(--accent)"
                  />
                  <Head user={f.user} size={24} ring="var(--bg)" />
                  <span class="min-w-0 flex-1 truncate"
                    >{f.user.displayName}</span
                  >
                </label>
              {/each}
            </div>
            <div class="mt-2 flex justify-end gap-2">
              <Button
                size="sm"
                variant="ghost"
                onclick={() => (inviting = null)}>Cancel</Button
              >
              <Button
                size="sm"
                variant="primary"
                disabled={busy || !picked.length}
                onclick={() => sendInvites(g)}
                >{picked.length
                  ? `Invite ${plural(picked.length, "friend")}`
                  : "Invite"}</Button
              >
            </div>
          {:else}
            <p class="text-[13px] text-(--text-muted)">
              All your friends are in {g.name}. Add friends with their code
              above, or copy a link for anyone else.
            </p>
          {/if}
        </div>
      {/if}
    {:else}
      <p class="py-2 text-[13px] text-(--text-muted)">No groups yet.</p>
    {/each}
  </div>
</div>
