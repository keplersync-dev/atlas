<script lang="ts">
  import { api } from "$lib/api";
  import type { InstanceSummary } from "$lib/types";
  import Button from "../Button.svelte";

  // A new instance's download waits here: its launcher (the Modrinth App) only takes the
  // instance's picture while it's closed.
  let {
    instance,
    compact = false,
  }: { instance: InstanceSummary; compact?: boolean } = $props();

  let closing = $state(false);
  let error = $state<string | null>(null);

  async function close() {
    closing = true;
    error = null;
    try {
      await api.closeLauncher(instance.id);
    } catch (e) {
      error = (e as Error).message;
    } finally {
      closing = false;
    }
  }

  async function skip() {
    error = null;
    try {
      await api.skipLauncherPicture(instance.id);
    } catch (e) {
      error = (e as Error).message;
    }
  }
</script>

<div class={compact ? "text-xs" : "text-[12.5px]"}>
  <p class="text-(--text-muted)">
    Close {instance.launcher.name} so Atlas can add this instance's picture there.
    The download starts once it's closed.
  </p>
  {#if error}
    <p class="mt-1 text-(--danger)" role="alert">{error}</p>
  {/if}
  <div class="mt-2 flex flex-wrap gap-1.5">
    <Button size="sm" variant="primary" disabled={closing} onclick={close}>
      {closing
        ? `Closing ${instance.launcher.name}…`
        : `Close ${instance.launcher.name} and download`}
    </Button>
    <Button size="sm" variant="ghost" onclick={skip}
      >Download without the picture</Button
    >
  </div>
</div>
