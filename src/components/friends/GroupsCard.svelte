<script lang="ts">
  import { api } from "$lib/api";
  import { copier } from "$lib/clipboard.svelte";
  import { confirmAction } from "$lib/confirm.svelte";
  import { openContextMenu } from "$lib/contextMenu.svelte";
  import { plural } from "$lib/format";
  import { personItems, presenceText } from "$lib/people";
  import { social } from "$lib/stores";
  import type { Group, GroupMember } from "$lib/types";
  import Button from "../Button.svelte";
  import Head from "../Head.svelte";
  import Icon from "../Icon.svelte";
  import Menu, { type MenuItem } from "../Menu.svelte";
  import PeoplePicker from "../PeoplePicker.svelte";

  const clip = copier(2200);

  let creating = $state(false);
  let name = $state("");
  let busy = $state(false);
  let message = $state<{ text: string; ok: boolean } | null>(null);
  // The group whose "Invite friends" picker is open.
  let open = $state<{ id: string; panel: "invite" } | null>(null);
  let picked = $state<string[]>([]);
  // Friends with an invite to the open group they haven't answered yet.
  let pending = $state<string[]>([]);

  // Atlas only knows owners and members. "admin" can still come from the server's API;
  // it shows as a plain member here.
  const manages = (g: Group) => g.myRole === "owner" || g.myRole === "admin";
  const me = $derived(social.session?.user?.id);

  /** Why a friend can't be picked for this group, or null when they can. */
  // A big group shows its first few members until asked, so the card stays scannable.
  const MEMBERS_SHOWN = 6;
  let allMembers = $state<Record<string, boolean>>({});

  const taken = (g: Group, id: string) =>
    g.members.some((m) => m.user.id === id)
      ? `In ${g.name}`
      : pending.includes(id)
        ? "Invite pending"
        : null;

  async function show(g: Group, panel: "invite") {
    open =
      open?.id === g.id && open.panel === panel ? null : { id: g.id, panel };
    picked = [];
    pending = [];
    message = null;
    if (!open) return;
    try {
      pending = (await api.getGroup(g.id)).invited ?? [];
    } catch {
      // Without it, pending friends are pickable; the server skips them anyway.
    }
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
      open = { id: g.id, panel: "invite" };
      pending = [];
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
        "They can then delete the group or remove anyone. You stay in it as a member, and can leave.",
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
      message: `${others ? `${plural(others, "person", "people")} ${others === 1 ? "leaves" : "leave"} with it. ` : ""}Packs shared with the group stop updating for those who only had them through it. Nobody's files are touched.`,
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

  /** A member's menu: the owner hands over and removes, and for anyone but you, the
   *  same report and block as everywhere else. */
  function memberItems(g: Group, m: GroupMember): MenuItem[] {
    return [
      ...(g.myRole === "owner" && m.user.id !== me
        ? [
            {
              label: "Make owner",
              hint: "You stay in the group as a member",
              icon: "crown",
              accent: true,
              onselect: () => handOver(g, m),
            },
          ]
        : []),
      ...(canRemove(g, m)
        ? [
            {
              label: `Remove from ${g.name}`,
              danger: true,
              onselect: () => remove(g, m),
            },
          ]
        : []),
      "divider",
      ...personItems(m.user),
    ];
  }

  /** Under a member's name: you, a friend's status, or their username. */
  function about(m: GroupMember) {
    if (m.user.id === me) return "You";
    const p = social.friends.find((f) => f.user.id === m.user.id)?.presence;
    return p ? presenceText(p) : `@${m.user.username}`;
  }
  const presenceOf = (m: GroupMember) =>
    social.friends.find((f) => f.user.id === m.user.id)?.presence.state ?? null;

  /** Each group's own colour for its tile, from its id, so groups are told apart. */
  const hue = (id: string) =>
    [...id].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7);

  /** Packs shared with the group that this PC knows about, by name. */
  const packsOf = (g: Group) =>
    social.shared.filter((s) => g.sharedInstanceIds.includes(s.id));

  /** Owners remove anyone; admins (from the API only) remove members. */
  const canRemove = (g: Group, m: GroupMember) =>
    m.user.id !== me &&
    (g.myRole === "owner" || (g.myRole === "admin" && m.role === "member"));
</script>

<!-- The section card the Shared page uses, so Friends and Groups read as one page. -->
<section class="rounded-xl border border-(--line) bg-(--bg-raise)">
  <header
    class="flex flex-wrap items-center gap-3 border-b border-(--line) px-4 py-3.5"
  >
    <span
      class="grid size-8 place-items-center rounded-lg bg-(--bg-raise-2) text-(--text-muted)"
    >
      <Icon name="group" class="size-4" />
    </span>
    <span class="min-w-40 flex-1">
      <b class="font-semibold">Groups</b>
      <span class="ml-1 text-(--text-faint)">{social.groups.length}</span>
      <small class="block text-[13px] text-(--text-faint)"
        >Share a pack with everyone in one, new members too.</small
      >
    </span>
    <Button
      size="sm"
      aria-pressed={creating}
      class={creating ? "border-(--accent)!" : ""}
      icon="plus"
      onclick={() => (creating = !creating)}
    >
      New group
    </Button>
  </header>

  {#if creating}
    <form
      onsubmit={create}
      class="flex gap-2 border-b border-(--line) px-4 py-3"
    >
      <input
        bind:value={name}
        maxlength="32"
        placeholder="Group name"
        aria-label="Group name"
        class="h-8 min-w-0 flex-1 rounded-md border border-(--line-2) bg-(--bg) px-3 text-[13px] outline-none placeholder:text-(--text-faint) focus:border-(--accent)"
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
      class="border-b border-(--line) px-4 py-2.5 text-[13px] {message.ok
        ? 'text-(--online)'
        : 'text-(--danger)'}"
      role="status"
    >
      {message.text}
    </p>
  {/if}

  <!-- Each group its own panel, with its own coloured tile, so two never read as one. -->
  <div class="grid gap-3 p-3">
    {#each social.groups as g (g.id)}
      <!-- svelte-ignore a11y_no_static_element_interactions (right-click repeats the ⋯ menu, which keyboards reach) -->
      <div
        class="rounded-lg border border-(--line) bg-(--bg)"
        oncontextmenu={(e) => openContextMenu(e, menu(g))}
      >
        <!-- Wraps: when narrow the buttons drop under the name. -->
        <div
          class="flex flex-wrap items-center gap-x-3 gap-y-2 px-3.5 pt-3 pb-2"
        >
          <div class="flex min-w-40 flex-1 items-center gap-3">
            <span
              aria-hidden="true"
              class="grid size-9 shrink-0 place-items-center rounded-lg text-[15px] font-semibold text-white"
              style="background: hsl({hue(g.id)} 42% 34%)"
              >{g.name.trim().charAt(0).toUpperCase()}</span
            >
            <div class="min-w-0 flex-1">
              <b class="block truncate font-semibold" title={g.name}>{g.name}</b
              >
              <small class="block truncate text-[13px] text-(--text-faint)">
                {plural(g.members.length, "member")} · {plural(
                  g.sharedInstanceIds.length,
                  "pack",
                )}{g.myRole === "owner" ? " · You own it" : ""}
              </small>
            </div>
          </div>
          <div class="ml-auto flex items-center gap-1">
            {#if manages(g)}
              <Button
                size="sm"
                aria-expanded={open?.id === g.id && open.panel === "invite"}
                disabled={busy}
                icon="users"
                onclick={() => show(g, "invite")}
              >
                Invite friends
              </Button>
            {/if}
            <Menu items={menu(g)} label="More for {g.name}" />
          </div>
        </div>

        <!-- Members as rows, like friends: what you can do is in ⋯ and on right-click. -->
        <div class="px-1.5 pb-1.5">
          {#each allMembers[g.id] ? g.members : g.members.slice(0, MEMBERS_SHOWN) as m (m.user.id)}
            {@const items = memberItems(g, m)}
            <!-- svelte-ignore a11y_no_static_element_interactions (right-click repeats the ⋯ menu, which keyboards reach) -->
            <div
              class="group flex items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-(--bg-hover)"
              oncontextmenu={(e) => openContextMenu(e, items)}
            >
              <Head
                user={m.user}
                size={28}
                presence={presenceOf(m)}
                ring="var(--bg)"
              />
              <div class="min-w-0 flex-1">
                <span class="flex min-w-0 items-center gap-2">
                  <b class="truncate font-medium">{m.user.displayName}</b>
                  {#if m.role === "owner"}
                    <span
                      class="flex shrink-0 items-center gap-1 rounded-full bg-(--bg-raise-2) px-2 text-[13px] text-(--text-muted)"
                      ><Icon name="crown" class="size-3" />Owner</span
                    >
                  {/if}
                </span>
                <small
                  class="block truncate text-[13px] {presenceOf(m) === 'playing'
                    ? 'text-(--online)'
                    : 'text-(--text-faint)'}">{about(m)}</small
                >
              </div>
              <!-- Shown on hover, or while focused for keyboards, like on friends. -->
              <div
                class="opacity-0 group-hover:opacity-100 focus-within:opacity-100"
              >
                <Menu {items} label="More for {m.user.displayName}" />
              </div>
            </div>
          {/each}
          {#if g.members.length > MEMBERS_SHOWN}
            <button
              type="button"
              class="w-full rounded-md px-2 py-1.5 text-left text-[13px] text-(--accent) hover:bg-(--bg-hover)"
              onclick={() => (allMembers[g.id] = !allMembers[g.id])}
              >{allMembers[g.id]
                ? "Show fewer"
                : `Show all ${g.members.length} members`}</button
            >
          {/if}
        </div>
        {#if packsOf(g).length}
          <p class="truncate px-3.5 pb-3 text-[13px] text-(--text-faint)">
            Packs:
            {#each packsOf(g) as p, i (p.id)}{#if i}{", "}{/if}<span
                class="text-(--text-muted)">{p.name}</span
              >{/each}
          </p>
        {/if}

        {#if manages(g) && open?.id === g.id && open.panel === "invite"}
          <!-- Every friend: those already in or invited are ticked and greyed, so an
               empty choice is plain to see rather than a message. -->
          {@const free = social.friends.filter((f) => !taken(g, f.user.id))}
          <div class="mx-3 mb-3">
            <PeoplePicker
              friends={social.friends}
              bind:friendIds={picked}
              friendTaken={(id) => taken(g, id)}
              empty="No friends yet. Add one with their friend code above, or copy an invite link from ⋯."
            />
            {#if social.friends.length && !free.length}
              <p class="px-2 pt-2 text-[13px] text-(--text-muted)">
                Everyone's in or invited. For anyone else, copy an invite link
                from ⋯.
              </p>
            {/if}
            <div class="mt-2 flex flex-wrap justify-end gap-2 px-1 pb-1">
              <Button size="sm" variant="ghost" onclick={() => (open = null)}
                >{free.length ? "Cancel" : "Close"}</Button
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
          </div>
        {/if}
      </div>
    {:else}
      <p class="px-1 py-3 text-[13px] text-(--text-muted)">
        No groups yet. Make one, then invite friends to it from here.
      </p>
    {/each}
  </div>
</section>
