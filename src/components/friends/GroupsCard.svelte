<script lang="ts">
  import { api } from "$lib/api";
  import { copier } from "$lib/clipboard.svelte";
  import { confirmAction } from "$lib/confirm.svelte";
  import { plural } from "$lib/format";
  import { social } from "$lib/stores";
  import type { Group, GroupMember, GroupRole } from "$lib/types";
  import Button from "../Button.svelte";
  import Head from "../Head.svelte";
  import Icon from "../Icon.svelte";
  import Menu, { type MenuItem } from "../Menu.svelte";

  const clip = copier(2200);

  let creating = $state(false);
  let name = $state("");
  let busy = $state(false);
  let message = $state<{ text: string; ok: boolean } | null>(null);
  // One panel open at a time, under its group: friends to invite, or the members.
  let open = $state<{ id: string; panel: "invite" | "members" } | null>(null);
  let picked = $state<string[]>([]);

  const ROLE: Record<GroupRole, string> = {
    owner: "Owner",
    admin: "Admin",
    member: "Member",
  };
  const manages = (g: Group) => g.myRole === "owner" || g.myRole === "admin";
  const invitable = (g: Group) =>
    social.friends.filter(
      (f) => !g.members.some((m) => m.user.id === f.user.id),
    );
  const me = $derived(social.session?.user?.id);

  function show(g: Group, panel: "invite" | "members") {
    open =
      open?.id === g.id && open.panel === panel ? null : { id: g.id, panel };
    picked = [];
    message = null;
  }

  async function run(action: () => Promise<string>) {
    busy = true;
    message = null;
    try {
      message = { text: await action(), ok: true };
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
      creating = false;
      // Straight on to the next step: who's in it.
      if (social.friends.length) open = { id: g.id, panel: "invite" };
      return `Created ${g.name}. Pick friends to invite, or copy a link.`;
    });
  }

  function sendInvites(g: Group) {
    run(async () => {
      await api.inviteFriendsToGroup(g.id, picked);
      const n = picked.length;
      open = null;
      picked = [];
      return `Invited ${plural(n, "friend")} to ${g.name}. They'll see it on their Friends page.`;
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
      message:
        "Packs you got through the group stop updating. Their files stay on your PC.",
      confirmLabel: "Leave group",
      danger: true,
    });
    if (!ok) return;
    run(async () => {
      await social.leaveGroup(g.id);
      return `You left ${g.name}.`;
    });
  }

  async function remove(g: Group, m: GroupMember) {
    const ok = await confirmAction({
      title: `Remove ${m.user.displayName} from ${g.name}?`,
      message:
        "Packs they got through the group stop updating for them. Their files stay on their PC.",
      confirmLabel: "Remove",
      danger: true,
    });
    if (!ok) return;
    run(async () => {
      await api.removeGroupMember(g.id, m.user.id);
      await social.load();
      return `Removed ${m.user.displayName} from ${g.name}.`;
    });
  }

  async function handOver(g: Group, m: GroupMember) {
    const ok = await confirmAction({
      title: `Make ${m.user.displayName} the owner of ${g.name}?`,
      message:
        "They can then delete the group or remove anyone. You stay in it as an admin, and can leave.",
      confirmLabel: "Make owner",
    });
    if (!ok) return;
    run(async () => {
      await api.transferGroup(g.id, m.user.id);
      await social.load();
      return `${m.user.displayName} now owns ${g.name}.`;
    });
  }

  async function deleteGroup(g: Group) {
    const others = g.members.length - 1;
    const ok = await confirmAction({
      title: `Delete ${g.name}?`,
      message: `${others ? `${plural(others, "person", "people")} leave with it. ` : ""}Packs shared with the group stop updating for those who only had them through it. Nobody's files are touched.`,
      confirmLabel: "Delete group",
      danger: true,
    });
    if (!ok) return;
    run(async () => {
      await api.deleteGroup(g.id);
      open = null;
      await social.load();
      return `Deleted ${g.name}.`;
    });
  }

  function menu(g: Group): MenuItem[] {
    return [
      { label: "Members", icon: "users", onselect: () => show(g, "members") },
      ...(manages(g)
        ? [
            {
              label: "Copy invite link",
              hint: "For people who aren't your friends yet",
              icon: "share",
              onselect: () => copyInvite(g),
            },
          ]
        : []),
      "divider",
      g.myRole === "owner"
        ? {
            label: "Delete group",
            hint: "Everyone leaves; nobody's files are touched",
            danger: true,
            onselect: () => deleteGroup(g),
          }
        : {
            label: "Leave group",
            danger: true,
            onselect: () => leave(g),
          },
    ];
  }

  /** What I can do to this member: owners remove anyone and hand over; admins remove members. */
  const canRemove = (g: Group, m: GroupMember) =>
    m.user.id !== me &&
    (g.myRole === "owner" || (g.myRole === "admin" && m.role === "member"));
</script>

<div class="rounded-xl border border-(--line) bg-(--bg-raise) p-4.5">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <h2 class="font-semibold">Groups</h2>
    <Button
      size="sm"
      aria-pressed={creating}
      class={creating ? "border-(--accent)!" : ""}
      onclick={() => (creating = !creating)}
    >
      <Icon name="plus" class="size-3.5" />
      New group
    </Button>
  </div>
  <p class="mt-1 mb-1.5 text-[13px] text-(--text-muted)">
    Share a pack with a whole group at once. New members get it automatically.
  </p>

  {#if creating}
    <form onsubmit={create} class="my-2 flex gap-2">
      <input
        bind:value={name}
        maxlength="32"
        placeholder="Group name"
        aria-label="Group name"
        class="h-8 min-w-0 flex-1 rounded-md border border-(--line-2) bg-(--bg) px-3 outline-none focus:border-(--accent)"
      />
      <Button
        type="submit"
        size="sm"
        variant="primary"
        disabled={busy || !name.trim()}>Create</Button
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
      <div class="py-2.5">
        <!-- Wraps: in the narrow column the button drops under the name. -->
        <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
          <div class="flex min-w-40 flex-1 items-center gap-3">
            <span class="flex shrink-0">
              {#each g.members.slice(0, 3) as m, i (m.user.id)}
                <span class={i ? "-ml-1.5" : ""}
                  ><Head user={m.user} size={22} ring="var(--bg-raise)" /></span
                >
              {/each}
            </span>
            <div class="min-w-0 flex-1">
              <b class="block truncate font-medium">{g.name}</b>
              <small class="block truncate text-[13px] text-(--text-faint)">
                {plural(g.members.length, "member")} · {plural(
                  g.sharedInstanceIds.length,
                  "pack",
                )} · {ROLE[g.myRole]}
              </small>
            </div>
          </div>
          <div class="ml-auto flex items-center gap-1">
            {#if manages(g)}
              <Button
                size="sm"
                aria-expanded={open?.id === g.id && open.panel === "invite"}
                disabled={busy}
                onclick={() => show(g, "invite")}
              >
                <Icon name="users" class="size-3.5" />Invite friends
              </Button>
            {/if}
            <Menu items={menu(g)} label="More for {g.name}" />
          </div>
        </div>

        {#if open?.id === g.id && open.panel === "invite"}
          {@const people = invitable(g)}
          <div class="mt-2 rounded-lg bg-(--bg) p-3">
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
              <div class="mt-2 flex flex-wrap justify-end gap-2">
                <Button size="sm" variant="ghost" onclick={() => (open = null)}
                  >Cancel</Button
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
                above, or copy a link for anyone else from the ⋯ menu.
              </p>
            {/if}
          </div>
        {:else if open?.id === g.id && open.panel === "members"}
          <div class="mt-2 grid gap-0.5 rounded-lg bg-(--bg) p-2">
            {#each g.members as m (m.user.id)}
              <!-- Actions sit under the name, so the row fits the narrow column. -->
              <div class="flex items-start gap-2.5 rounded-md px-2 py-1.5">
                <Head user={m.user} size={24} ring="var(--bg)" />
                <div class="min-w-0 flex-1">
                  <span class="block truncate">
                    {m.user.displayName}{m.user.id === me ? " (you)" : ""}
                    <small class="ml-1 text-[13px] text-(--text-faint)"
                      >{ROLE[m.role]}</small
                    >
                  </span>
                  {#if (g.myRole === "owner" && m.user.id !== me) || canRemove(g, m)}
                    <span class="-ml-1.5 flex flex-wrap gap-1">
                      {#if g.myRole === "owner" && m.user.id !== me}
                        <button
                          type="button"
                          disabled={busy}
                          onclick={() => handOver(g, m)}
                          class="rounded px-1.5 py-0.5 text-[13px] text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
                          >Make owner</button
                        >
                      {/if}
                      {#if canRemove(g, m)}
                        <button
                          type="button"
                          disabled={busy}
                          onclick={() => remove(g, m)}
                          class="rounded px-1.5 py-0.5 text-[13px] text-(--danger) hover:bg-(--bg-hover)"
                          >Remove</button
                        >
                      {/if}
                    </span>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {:else}
      <p class="py-2 text-[13px] text-(--text-muted)">
        No groups yet. Make one, and invite friends to it from here.
      </p>
    {/each}
  </div>
</div>
