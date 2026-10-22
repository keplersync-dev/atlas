<script lang="ts">
  import { api } from "$lib/api";
  import { formatCode, plural } from "$lib/format";
  import { go } from "$lib/nav.svelte";
  import { resource } from "$lib/resource.svelte";
  import { social } from "$lib/stores";
  import type { OpenedLink } from "$lib/types";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";
  import Head from "../Head.svelte";

  // Atlas was opened with a friend or group link. Show who or what it leads to; nothing
  // happens until the user clicks.
  let { link, onclose }: { link: OpenedLink; onclose: () => void } = $props();

  const friend = resource(() =>
    link.kind === "friend" && social.signedIn
      ? api.previewFriendCode(link.code)
      : Promise.resolve(null),
  );
  const group = resource(() =>
    link.kind === "group" && social.signedIn
      ? api.previewGroupInvite(link.code)
      : Promise.resolve(null),
  );

  // What the person already is to you decides the button.
  const relation = $derived.by(() => {
    const id = friend.data?.id;
    if (!id) return null;
    if (id === social.session?.user?.id) return "self";
    if (social.friends.some((f) => f.user.id === id)) return "friend";
    if (social.incomingRequests.some((r) => r.user.id === id))
      return "incoming";
    if (social.outgoingRequests.some((r) => r.user.id === id)) return "pending";
    return "new";
  });

  let busy = $state(false);
  let error = $state<string | null>(null);

  async function act(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = null;
    try {
      if (link.kind === "friend") {
        await social.sendFriendRequest(link.code);
      } else {
        await social.joinGroup(link.code);
      }
      onclose();
      go({ page: "friends" });
    } catch (err) {
      error = (err as Error).message;
    } finally {
      busy = false;
    }
  }

  const loading = $derived(friend.loading || group.loading);
  const lookupError = $derived(friend.error ?? group.error);
  const canAct = $derived(
    !busy &&
      ((link.kind === "friend" &&
        (relation === "new" || relation === "incoming")) ||
        (link.kind === "group" && !!group.data && !group.data.alreadyMember)),
  );
</script>

<Dialog
  title={link.kind === "friend" ? "Add a friend" : "Join a group"}
  description={link.kind === "friend"
    ? `Friend code ${formatCode(link.code)}`
    : "From an invite link"}
  {onclose}
  onsubmit={act}
>
  {#if !social.signedIn}
    <p class="py-2 text-[12.5px] text-(--text-muted)">
      Sign in first, then open the link again.
    </p>
  {:else if loading}
    <p class="py-2 text-[12.5px] text-(--text-muted)">Looking it up…</p>
  {:else if lookupError}
    <p class="py-2 text-[12.5px] text-(--danger)">{lookupError}</p>
  {:else if friend.data}
    <div class="flex items-center gap-3 rounded-lg bg-(--bg-raise) px-3 py-3">
      <Head user={friend.data} size={36} ring="var(--bg-raise)" />
      <div class="min-w-0 flex-1">
        <b class="block font-semibold">{friend.data.displayName}</b>
        <small class="text-[11.5px] text-(--text-faint)"
          >@{friend.data.username}</small
        >
      </div>
      <span class="text-xs text-(--text-muted)">
        {#if relation === "self"}That's you
        {:else if relation === "friend"}Already friends
        {:else if relation === "pending"}Request sent
        {:else if relation === "incoming"}Wants to be friends
        {/if}
      </span>
    </div>
  {:else if group.data}
    <div class="rounded-lg bg-(--bg-raise) px-3.5 py-3">
      <b class="block text-[14px] font-semibold">{group.data.groupName}</b>
      <small class="text-[11.5px] text-(--text-faint)">
        {plural(group.data.memberCount, "member")}{group.data.invitedBy
          ? ` · invite from ${group.data.invitedBy.displayName}`
          : ""}
      </small>
    </div>
    <p class="mt-2.5 text-xs text-(--text-muted)">
      {group.data.alreadyMember
        ? "You're already in this group."
        : "Instances shared with the group will show up for you to add."}
    </p>
  {/if}

  {#snippet footer()}
    {#if error}<span class="mr-auto text-xs text-(--danger)" role="alert"
        >{error}</span
      >{/if}
    <Button variant="ghost" onclick={onclose}>
      {canAct ? "Cancel" : "Close"}
    </Button>
    {#if canAct}
      <Button type="submit" variant="primary" disabled={busy}>
        {#if link.kind === "group"}Join group
        {:else if relation === "incoming"}Accept their request
        {:else}Send friend request{/if}
      </Button>
    {/if}
  {/snippet}
</Dialog>
