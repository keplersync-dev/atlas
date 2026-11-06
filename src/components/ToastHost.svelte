<script lang="ts">
  import { openDialog } from "$lib/dialogs.svelte";
  import { go } from "$lib/nav.svelte";
  import { dismissToast, toasts } from "$lib/toasts.svelte";
  import type { NoticeTarget } from "$lib/types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  // Rendered once in the app shell. See $lib/toasts.svelte.ts.

  // An open dialog sits in the browser's top layer, above everything else. The toasts go
  // there too (a popover), shown again on each change so they stay above a newer dialog.
  let host = $state<HTMLDivElement>();
  $effect(() => {
    const showing = toasts.items.length > 0;
    if (!host) return;
    if (host.matches(":popover-open")) host.hidePopover();
    if (showing) host.showPopover();
  });

  function open(id: number, target: NoticeTarget) {
    dismissToast(id);
    switch (target.kind) {
      case "pack":
        return go({ page: "instance", id: target.instanceId });
      case "review":
        return go({ page: "review", id: target.instanceId });
      case "invite":
        return openDialog({
          kind: "accept",
          sharedInstanceId: target.sharedInstanceId,
        });
      case "friends":
        return go({ page: "friends" });
      case "home":
        return go({ page: "home" });
    }
  }
</script>

<div
  bind:this={host}
  popover="manual"
  class="pointer-events-none fixed inset-auto right-4 bottom-4 z-50 m-0 grid w-[min(380px,calc(100vw-32px))] gap-2 overflow-visible border-0 bg-transparent p-0"
>
  {#each toasts.items as t (t.id)}
    <div
      role={t.kind === "error" ? "alert" : "status"}
      class="pointer-events-auto flex items-start gap-2.5 rounded-lg border px-3.5 py-2.5 text-[13px] shadow-[0_12px_32px_#0008]
        {t.kind === 'error'
        ? 'border-(--danger)/30 bg-[#1d1416]'
        : 'border-(--accent)/35 bg-(--bg-raise-2)'}"
    >
      {#if t.kind === "error"}
        <Icon name="warn" class="mt-0.5 size-4 shrink-0 text-(--danger)" />
        <p class="min-w-0 flex-1">{t.text}</p>
      {:else if t.kind === "done"}
        <Icon name="check" class="mt-0.5 size-4 shrink-0 text-(--online)" />
        <p class="min-w-0 flex-1">{t.text}</p>
      {:else}
        <Icon name="inbox" class="mt-0.5 size-4 shrink-0 text-(--accent)" />
        <div class="min-w-0 flex-1">
          <b class="block text-[13px] font-semibold">{t.notice.title}</b>
          <p class="mt-0.5 text-(--text-muted)">{t.notice.body}</p>
          <Button
            size="sm"
            class="mt-2"
            onclick={() => open(t.id, t.notice.target)}>Open</Button
          >
        </div>
      {/if}
      <button
        aria-label="Dismiss"
        onclick={() => dismissToast(t.id)}
        class="-m-0.5 grid size-6 shrink-0 place-items-center rounded text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
      >
        <Icon name="close" class="size-3" />
      </button>
    </div>
  {/each}
</div>
