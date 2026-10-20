<script lang="ts">
  import { openDialog } from "$lib/dialogs.svelte";
  import { fileSize, plural, timeAgo, versionLabel } from "$lib/format";
  import { social } from "$lib/stores";
  import type { SharedInstance } from "$lib/types";
  import BlockArt from "./BlockArt.svelte";
  import Button from "./Button.svelte";
  import Head from "./Head.svelte";
  import Icon from "./Icon.svelte";

  // An instance a friend shared that you haven't added yet.
  let { invite }: { invite: SharedInstance } = $props();

  const from = $derived(invite.invitedBy ?? invite.owner);
  let declining = $state(false);

  async function decline() {
    declining = true;
    try {
      await social.declineInvite(invite.id);
    } finally {
      declining = false;
    }
  }
</script>

<article class="flex items-center gap-3.5 rounded-xl bg-(--bg-raise) p-3.5">
  <BlockArt seed={invite.id} class="size-12 rounded-lg" />
  <div class="min-w-0 flex-1">
    <b class="block truncate text-[14px] font-semibold">{invite.name}</b>
    <span class="mt-0.5 flex items-center gap-1.5 text-xs text-(--text-muted)">
      {#if from}<Head user={from} size={16} ring="var(--bg-raise)" />{/if}
      <span class="truncate">
        {from?.displayName ?? "Someone"}{invite.myViaGroup
          ? ` · to ${invite.myViaGroup.name}`
          : ""} · {timeAgo(invite.invitedAt)}
      </span>
    </span>
    <small class="block truncate text-[11.5px] text-(--text-faint)">
      {versionLabel(invite.loader, invite.minecraftVersion)} · {plural(
        invite.contentCount,
        "item",
      )} · {fileSize(invite.sizeBytes)}
    </small>
  </div>
  <div class="flex shrink-0 items-center gap-1">
    <Button
      size="sm"
      variant="primary"
      onclick={() =>
        openDialog({ kind: "accept", sharedInstanceId: invite.id })}>Add</Button
    >
    <button
      onclick={decline}
      disabled={declining}
      title="Decline"
      aria-label="Decline {invite.name}"
      class="grid size-7 place-items-center rounded-md text-(--text-faint) hover:bg-(--bg-hover) hover:text-(--text-primary) disabled:opacity-40"
    >
      <Icon name="close" class="size-3.5" />
    </button>
  </div>
</article>
