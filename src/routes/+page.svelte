<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { nav } from "$lib/nav.svelte";
  import { connectStores } from "$lib/stores";
  import TitleBar from "../components/TitleBar.svelte";
  import Sidebar from "../components/Sidebar.svelte";
  import DialogHost from "../components/DialogHost.svelte";
  import Home from "../tabs/Home.svelte";
  import Shared from "../tabs/Shared.svelte";
  import Friends from "../tabs/Friends.svelte";
  import Instance from "../tabs/Instance.svelte";
  import Review from "../tabs/Review.svelte";

  // Load all shared data once and keep it live. The returned cleanup
  // unsubscribes from events if this component is ever destroyed.
  onMount(connectStores);

  // The window starts hidden so it never shows a blank page while this loads; show it
  // once the first frame is drawn.
  onMount(() => {
    if ("__TAURI_INTERNALS__" in window) {
      requestAnimationFrame(() => void getCurrentWindow().show());
    }
  });
</script>

<!-- prevent default right click behaviour -->
<svelte:window oncontextmenu={(e) => e.preventDefault()} />

<div class="grid h-screen grid-rows-[auto_1fr]">
  <TitleBar />

  <div class="grid min-h-0 grid-cols-[236px_1fr]">
    <Sidebar />

    <main class="min-h-0 overflow-y-auto">
      {#if nav.view.page === "home"}
        <!-- Home uses the full width and lays out its own columns. -->
        <Home />
      {:else}
        <!-- Other pages use the full width too; a page that reads better narrow
             (like Review) sets its own max width. -->
        <div class="px-9 pt-7 pb-16">
          {#if nav.view.page === "shared"}
            <Shared />
          {:else if nav.view.page === "friends"}
            <Friends />
          {:else if nav.view.page === "instance"}
            <Instance id={nav.view.id} />
          {:else if nav.view.page === "review"}
            <Review id={nav.view.id} />
          {/if}
        </div>
      {/if}
    </main>
  </div>
</div>

<!-- Whichever dialog is open (see $lib/dialogs.svelte.ts) renders here, above everything. -->
<DialogHost />
