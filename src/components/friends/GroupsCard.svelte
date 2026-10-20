<script lang="ts">
  import { api } from "$lib/api";
  import { copier } from "$lib/clipboard.svelte";
  import { plural } from "$lib/format";
  import { social } from "$lib/stores";
  import type { Group } from "$lib/types";
  import Button from "../Button.svelte";
  import Head from "../Head.svelte";

  const clip = copier(2200);

  // Which small inline form is open. One variable instead of two booleans:
  // opening one closes the other automatically.
  let form = $state<"none" | "create" | "join">("none");
  let name = $state("");
  let joinCode = $state("");
  let busy = $state(false);
  let message = $state<{ text: string; ok: boolean } | null>(null);

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
      return `Created ${g.name}. Copy its invite link to add people.`;
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

  function leave(g: Group) {
    if (
      !confirm(
        `Leave ${g.name}? Instances shared with the group stay on your PC.`,
      )
    )
      return;
    run(async () => {
      await social.leaveGroup(g.id);
      return `You left ${g.name}.`;
    });
  }
</script>

<div class="rounded-xl bg-(--bg-raise) p-4.5">
  <div class="flex items-baseline justify-between">
    <h2 class="font-semibold">Groups</h2>
    <div class="flex gap-3 text-xs">
      <button
        class="text-(--accent) hover:underline"
        onclick={() => (form = form === "join" ? "none" : "join")}>Join</button
      >
      <button
        class="text-(--accent) hover:underline"
        onclick={() => (form = form === "create" ? "none" : "create")}
        >New group</button
      >
    </div>
  </div>
  <p class="mt-1 mb-1.5 text-[12.5px] text-(--text-muted)">
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
        class="h-8 min-w-0 flex-1 rounded-md border border-(--line-2) bg-(--bg) px-3 font-mono text-xs outline-none focus:border-(--accent)"
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
      class="my-1.5 text-xs {message.ok
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
          <small class="text-[11.5px] text-(--text-faint) capitalize">
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
            onclick={() => copyInvite(g)}
          >
            {clip.copied === g.id ? "Copied" : "Invite link"}
          </Button>
        {/if}
      </div>
    {:else}
      <p class="py-2 text-[12.5px] text-(--text-muted)">No groups yet.</p>
    {/each}
  </div>
</div>
