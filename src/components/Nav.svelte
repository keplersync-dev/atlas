<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import { tabs, type TabComponent } from "../lib/tabs";
  import NavButton from "./NavButton.svelte";

  let {
    class: className = "",
    activeComponent = $bindable<TabComponent>(),
    ...props
  } = $props();

  const win = getCurrentWindow();

  let isMaximised = $state(false);

  const windowActions = $derived([
    {
      label: "Minimize",
      icon: "/minimise.svg",
      destructive: false,
      run: () => win.minimize(),
    },
    {
      label: "Maximize",
      icon: isMaximised ? "/restore.svg" : "/maximise.svg",
      destructive: false,
      run: () => win.toggleMaximize(),
    },
    {
      label: "Close",
      icon: "/close.svg",
      destructive: true,
      run: () => win.close(),
    },
  ]);

  onMount(() => {
    const updateMaximizedState = async () => {
      isMaximised = await win.isMaximized();
    };

    updateMaximizedState();

    const unlisten = win.onResized(updateMaximizedState);

    return () => {
      unlisten.then((fn) => fn());
    };
  });
</script>

<header
  data-tauri-drag-region
  {...props}
  class="
    sticky top-0
    flex h-14 min-w-screen items-center
    border-b border-(--border)
    bg-(--bg-titlebar)
    pl-5
    [&_img]:pointer-events-none
    {className}
  "
>
  <!-- Branding -->
  <div
    data-tauri-drag-region
    class="
    mr-6
    flex items-baseline gap-2
    text-[17px] leading-none
    font-semibold tracking-tight select-none
  "
  >
    <!-- items-baseline puts the bottom of the mark on the text baseline -->
    <img
      data-tauri-drag-region
      src="/atlas-mark.svg"
      alt=""
      class="h-5 w-auto"
    />

    <span data-tauri-drag-region>Atlas</span>
  </div>

  <!-- Navigation -->
  <nav
    data-tauri-drag-region
    class="flex h-full flex-1 items-stretch gap-1.25 [&_img]:invert"
  >
    {#each tabs as { icon, label, component }}
      <NavButton
        selected={activeComponent === component}
        onclick={() => (activeComponent = component)}
      >
        <img src={icon} alt="" />
        {label}
      </NavButton>
    {/each}
  </nav>

  <div class="flex items-center gap-3">
    <!-- Add instance -->
    <button
      class="
    flex h-9.5 items-center
    gap-2
    rounded-lg border
    border-(--button-border) px-3
    py-2
    text-sm font-medium
    text-(--text-primary)
    transition-colors
    hover:border-[#60717d]
    hover:bg-(--bg-hover)
    
  "
    >
      <img src="/plus.svg" alt="" class="size-4.5 invert" />
      <span class="">Add instance</span>
    </button>

    <!-- Login -->
    <!-- logged in -->
    <button
      class="
    flex h-9.5 items-center gap-2
    rounded-lg border border-(--button-border)
    px-2.5
    text-sm text-(--text-primary)
    transition-colors
    hover:border-[#536b76]
    hover:bg-(--bg-hover)
  "
    >
      <div
        class="
      flex size-6 shrink-0 items-center justify-center
      rounded-full
      bg-[#26323a]
      text-xs font-medium
      text-(--accent)
    "
      >
        C
      </div>

      <span class="truncate">Alex</span>
    </button>
    <!-- to login -->
    <button
      class="
    flex h-9.5 items-center gap-2
    rounded-lg border border-(--button-border)
    px-2.5
    text-sm text-(--text-muted)
    transition-colors
    hover:border-[#536b76]
    hover:bg-(--bg-hover)
    hover:text-(--text-primary)
  "
      aria-label="Log in"
    >
      <div
        class="
      flex size-6 shrink-0 items-center justify-center
      rounded-full
      bg-[#20282f]
      text-(--text-muted)
    "
      >
        <img src="/login.svg" alt="" class="size-3.5 opacity-70 invert" />
      </div>

      <span>Log in</span>
    </button>

    <div class="mx-2 h-5 w-px bg-(--border) opacity-70"></div>
  </div>

  <!-- Window controls -->
  <div class="flex h-full items-stretch select-none">
    {#each windowActions as { label, icon, destructive, run }}
      <button
        onclick={run}
        aria-label={`${label} window`}
        class="
          group
          flex items-center justify-center
          px-3
          transition-colors duration-200

          {destructive ? 'hover:bg-[#ef4444]' : 'hover:bg-(--bg-hover)'}
        "
      >
        <span
          class="
            size-4.5
            bg-(--text-muted)
            transition-colors duration-200
            group-hover:bg-white
          "
          style="
            mask: url({icon}) no-repeat center / contain;
            -webkit-mask: url({icon}) no-repeat center / contain;
          "
        ></span>
      </button>
    {/each}
  </div>
</header>
