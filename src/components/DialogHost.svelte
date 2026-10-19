<script lang="ts">
  import { closeDialog, dialogs } from "$lib/dialogs.svelte";
  import AcceptInviteDialog from "./dialogs/AcceptInviteDialog.svelte";
  import AccountDialog from "./dialogs/AccountDialog.svelte";
  import PublishDialog from "./dialogs/PublishDialog.svelte";
  import ShareDialog from "./dialogs/ShareDialog.svelte";
  import TrackDialog from "./dialogs/TrackDialog.svelte";

  // Rendered once in the app shell. Whatever dialogs.active says is shown;
  // closing removes it from the page (see Dialog.svelte).
  const active = $derived(dialogs.active);
</script>

<!-- {#key} remounts the dialog when a different one opens, so its state starts fresh. -->
{#key active}
  {#if active?.kind === "share" || active?.kind === "invite"}
    <ShareDialog
      instanceId={active.instanceId}
      mode={active.kind}
      onclose={closeDialog}
    />
  {:else if active?.kind === "publish"}
    <PublishDialog instanceId={active.instanceId} onclose={closeDialog} />
  {:else if active?.kind === "accept"}
    <AcceptInviteDialog
      sharedInstanceId={active.sharedInstanceId}
      onclose={closeDialog}
    />
  {:else if active?.kind === "track"}
    <TrackDialog onclose={closeDialog} />
  {:else if active?.kind === "account"}
    <AccountDialog onclose={closeDialog} />
  {/if}
{/key}
