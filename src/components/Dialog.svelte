<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  // A modal built on the native <dialog> element. The browser gives us focus
  // trapping, Escape to close, and the backdrop for free.
  //
  // Rule of thumb: if this component is rendered, the dialog is open. To close it,
  // the parent stops rendering it (onclose tells the parent when the user closed it).
  let {
    title,
    description,
    wide = false,
    onclose,
    onsubmit,
    header,
    children,
    footer,
  }: {
    title: string;
    description?: string;
    wide?: boolean;
    onclose: () => void;
    /** If given, the body and footer are wrapped in a <form>: Enter and type="submit" buttons submit it. */
    onsubmit?: (e: SubmitEvent) => void;
    /** Optional picture left of the title. */
    header?: Snippet;
    children: Snippet;
    footer?: Snippet;
  } = $props();

  let el: HTMLDialogElement;

  // showModal() is a DOM call, so it has to wait until the element exists.
  onMount(() => el.showModal());

  // A click whose target is the <dialog> itself landed on the backdrop
  // (clicks inside hit the content div instead).
  function onclick(e: MouseEvent) {
    if (e.target === el) el.close();
  }
</script>

{#snippet body()}
  <div class="overflow-y-auto px-5.5 pt-3.5 pb-4.5">{@render children()}</div>
  {#if footer}
    <div
      class="flex items-center justify-end gap-2 border-t border-(--line) px-5.5 py-3.5"
    >
      {@render footer()}
    </div>
  {/if}
{/snippet}

<!-- The native "close" event fires for Escape, el.close() and form method="dialog". -->
<dialog
  bind:this={el}
  {onclick}
  {onclose}
  aria-labelledby="dialog-title"
  class="m-auto max-h-[calc(100vh-80px)] w-full rounded-xl border border-(--line-2) bg-[#15181d] p-0 text-(--text-primary) shadow-[0_30px_80px_#000a]
    backdrop:bg-[#05070acc] {wide ? 'max-w-155' : 'max-w-130'}"
>
  <div class="flex max-h-[calc(100vh-82px)] flex-col">
    <div class="flex items-start gap-3.5 px-5.5 pt-5 pb-1.5">
      {@render header?.()}
      <div class="min-w-0 flex-1">
        <h2 id="dialog-title" class="text-base font-semibold">{title}</h2>
        {#if description}<p class="mt-0.5 text-[12.5px] text-(--text-muted)">
            {description}
          </p>{/if}
      </div>
      <button
        aria-label="Close"
        onclick={() => el.close()}
        class="grid size-8 place-items-center rounded-md text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)"
      >
        <Icon name="close" class="size-3.5" />
      </button>
    </div>

    {#if onsubmit}
      <form {onsubmit} class="flex min-h-0 flex-col">{@render body()}</form>
    {:else}
      {@render body()}
    {/if}
  </div>
</dialog>
