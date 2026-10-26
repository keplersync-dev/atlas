<script lang="ts">
  import { dismissToast, toasts } from "$lib/toasts.svelte";
  import Icon from "./Icon.svelte";

  // Rendered once in the app shell. See $lib/toasts.svelte.ts.
</script>

<div
  class="pointer-events-none fixed right-4 bottom-4 z-50 grid w-[min(380px,calc(100vw-32px))] gap-2"
>
  {#each toasts.items as t (t.id)}
    <div
      role="alert"
      class="pointer-events-auto flex items-start gap-2.5 rounded-lg border border-(--danger)/30 bg-[#1d1416] px-3.5 py-2.5 text-[12.5px] shadow-[0_12px_32px_#0008]"
    >
      <Icon name="warn" class="mt-0.5 size-4 shrink-0 text-(--danger)" />
      <p class="min-w-0 flex-1">{t.text}</p>
      <button
        aria-label="Dismiss"
        onclick={() => dismissToast(t.id)}
        class="grid size-5 shrink-0 place-items-center rounded text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
      >
        <Icon name="close" class="size-3" />
      </button>
    </div>
  {/each}
</div>
