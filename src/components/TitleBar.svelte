<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { dialogs } from "$lib/dialogs.svelte";
  import { back } from "$lib/nav.svelte";
  import Icon from "./Icon.svelte";

  // Outside Tauri (plain `pnpm dev` in a browser) there is no window to control.
  const win = "__TAURI_INTERNALS__" in window ? getCurrentWindow() : null;

  let isMaximised = $state(false);

  const windowActions = $derived([
    {
      label: "Minimize",
      icon: "minimise",
      destructive: false,
      run: () => win?.minimize(),
    },
    {
      label: "Maximize",
      icon: isMaximised ? "restore" : "maximise",
      destructive: false,
      run: () => win?.toggleMaximize(),
    },
    {
      label: "Close",
      icon: "close",
      destructive: true,
      run: () => win?.close(),
    },
  ]);

  // Back like a browser: the mouse's side "back" button (button 3) and Alt+←.
  // Ignored while a dialog is open, so you can't navigate away underneath it.
  function onMouseUp(e: MouseEvent) {
    if (e.button === 3 && !dialogs.active) {
      e.preventDefault();
      back();
    }
  }
  function onKeyDown(e: KeyboardEvent) {
    if (e.altKey && e.key === "ArrowLeft" && !dialogs.active) {
      e.preventDefault();
      back();
    }
  }

  onMount(() => {
    if (!win) return;
    const update = async () => {
      isMaximised = await win.isMaximized();
    };
    update();
    const unlisten = win.onResized(update);
    return () => {
      unlisten.then((fn) => fn());
    };
  });
</script>

<svelte:window onmouseup={onMouseUp} onkeydown={onKeyDown} />

<header
  data-tauri-drag-region
  class="flex h-9 shrink-0 items-center border-b border-white/3 bg-(--bg-side) pl-3.5 select-none"
>
  <div
    data-tauri-drag-region
    class="flex items-center gap-2 text-[12.5px] font-semibold text-(--text-muted)"
  >
    <img
      data-tauri-drag-region
      src="/atlas-mark.svg"
      alt=""
      class="pointer-events-none h-3.5 w-auto"
    />
    <span data-tauri-drag-region>Atlas</span>
  </div>

  <div data-tauri-drag-region class="flex-1 self-stretch"></div>

  <div class="flex self-stretch">
    {#each windowActions as { label, icon, destructive, run } (label)}
      <button
        onclick={run}
        aria-label="{label} window"
        class="grid w-11.5 place-items-center text-(--text-faint) transition-colors duration-150 hover:text-white
          {destructive ? 'hover:bg-[#c42b1c]' : 'hover:bg-(--bg-hover)'}"
      >
        <Icon name={icon} class="size-4" />
      </button>
    {/each}
  </div>
</header>
