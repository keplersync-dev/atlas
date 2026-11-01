<script lang="ts">
  import { api } from "$lib/api";
  import { copier } from "$lib/clipboard.svelte";
  import { confirmAction } from "$lib/confirm.svelte";
  import { plural } from "$lib/format";
  import { social } from "$lib/stores";
  import type { Group, GroupMember } from "$lib/types";
  import Button from "../Button.svelte";
  import Head from "../Head.svelte";
  import Icon from "../Icon.svelte";
  import Menu, { type MenuItem } from "../Menu.svelte";

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

  /** What I can do to a member, as a menu: the owner hands over and removes anyone.
   *  Empty when there's nothing to do (then the chip isn't a button). */
  function memberMenu(g: Group, m: GroupMember): MenuItem[] {
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
      "divider",
      ...(canRemove(g, m)
        ? [
            {
              label: `Remove ${m.user.displayName}`,
              danger: true,
              onselect: () => remove(g, m),
            },
          ]
        : []),
    ];
  }

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
      onclick={() => (creating = !creating)}
    >
      <Icon name="plus" class="size-3.5" />
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

  <div class="divide-y divide-(--line)">
    {#each social.groups as g (g.id)}
      <div class="px-4 py-3">
        <!-- Wraps: when narrow the buttons drop under the name. -->
        <div class="flex flex-wrap items-center gap-x-3 gap-y-2">
          <div class="min-w-40 flex-1">
            <b class="block truncate font-medium">{g.name}</b>
            <small class="block truncate text-[13px] text-(--text-faint)">
              {plural(g.members.length, "member")} · {plural(
                g.sharedInstanceIds.length,
                "pack",
              )}{g.myRole === "owner" ? " · You own it" : ""}
            </small>
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

        <!-- Members, always in view; click one for what you can do. -->
        <div class="mt-2.5 flex flex-wrap gap-1.5">
          {#each g.members as m (m.user.id)}
            {@const items = memberMenu(g, m)}
            {#snippet chip()}
              <Head user={m.user} size={20} ring="var(--bg-raise)" />
              <span class="max-w-36 truncate">{m.user.displayName}</span>
              {#if m.role === "owner"}<span class="text-(--text-faint)"
                  >Owner</span
                >{/if}
              {#if m.user.id === me}<span class="text-(--text-faint)"
                  >(you)</span
                >{/if}
            {/snippet}
            {#if items.length}
              <Menu
                {items}
                align="left"
                label="{m.user.displayName}: options"
                trigger={chip}
                triggerClass="flex h-7 items-center gap-1.5 rounded-full border border-(--line) bg-(--bg) py-0.5 pr-2.5 pl-1 text-[13px] hover:border-(--line-2) hover:bg-(--bg-hover)"
              />
            {:else}
              <span
                class="flex h-7 items-center gap-1.5 rounded-full border border-(--line) bg-(--bg) py-0.5 pr-2.5 pl-1 text-[13px]"
                >{@render chip()}</span
              >
            {/if}
          {/each}
        </div>
        {#if packsOf(g).length}
          <p class="mt-2 truncate text-[13px] text-(--text-faint)">
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
          <div class="mt-3 rounded-lg bg-(--bg) p-2">
            {#each social.friends as f (f.user.id)}
              {@const why = taken(g, f.user.id)}
              <label
                class="flex items-center gap-2.5 rounded-md px-2 py-1.5 {why
                  ? 'opacity-50'
                  : 'cursor-pointer hover:bg-(--bg-hover)'}"
              >
                {#if why}
                  <input
                    type="checkbox"
                    checked
                    disabled
                    class="size-3.75 accent-(--accent)"
                  />
                {:else}
                  <input
                    type="checkbox"
                    bind:group={picked}
                    value={f.user.id}
                    class="size-3.75 accent-(--accent)"
                  />
                {/if}
                <Head user={f.user} size={28} ring="var(--bg)" />
                <span class="min-w-0 flex-1">
                  <b class="block truncate font-medium">{f.user.displayName}</b>
                  <small class="block truncate text-[13px] text-(--text-faint)"
                    >{why ?? `@${f.user.username}`}</small
                  >
                </span>
              </label>
            {:else}
              <p class="px-2 py-1.5 text-[13px] text-(--text-muted)">
                No friends yet. Add one with their friend code above, or copy an
                invite link from ⋯.
              </p>
            {/each}
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
      <p class="px-4 py-6 text-[13px] text-(--text-muted)">
        No groups yet. Make one, then invite friends to it from here.
      </p>
    {/each}
  </div>
</section>
