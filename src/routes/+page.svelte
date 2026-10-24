<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { nav } from "$lib/nav.svelte";
  import { connectStores, social } from "$lib/stores";
  import Icon from "../components/Icon.svelte";
  import TitleBar from "../components/TitleBar.svelte";
  import Sidebar from "../components/Sidebar.svelte";
  import DialogHost from "../components/DialogHost.svelte";
  import ConfirmHost from "../components/ConfirmHost.svelte";
  import Home from "../tabs/Home.svelte";
  import Shared from "../tabs/Shared.svelte";
  import Friends from "../tabs/Friends.svelte";
  import Instance from "../tabs/Instance.svelte";
  import Review from "../tabs/Review.svelte";

  // Load all shared data once and keep it live. The returned cleanup
  // unsubscribes from events if this component is ever destroyed.
  onMount(connectStores);

  // The window starts hidden so it never shows a blank page while this loads; show it
  // once the first frame is drawn (it stays in the tray when Atlas started with Windows).
  onMount(() => {
    if ("__TAURI_INTERNALS__" in window) {
      requestAnimationFrame(() => void api.windowReady());
    }
  });
</script>

<!-- prevent default right click behaviour -->
<svelte:window oncontextmenu={(e) => e.preventDefault()} />

<div class="grid h-screen grid-rows-[auto_1fr]">
  <TitleBar />

  <div class="grid min-h-0 grid-cols-[236px_1fr]">
    <Sidebar />

    <div class="flex min-h-0 flex-col">
      {#if social.session && !social.session.online}
        <div
          role="status"
          class="flex items-start gap-3 border-b border-(--warn)/25 bg-(--warn)/10 px-9 py-2.5 text-[12.5px]"
        >
          <Icon name="warn" class="mt-0.5 size-4 text-(--warn)" />
          <p>
            <b class="font-medium">Can't reach the Atlas server right now.</b>
            <span class="text-(--text-muted)">
              It's not something on your end. Your instances still work, and
              friends and sharing come back by themselves once the server is
              reachable.
            </span>
          </p>
        </div>
      {/if}
      <main class="min-h-0 flex-1 overflow-y-auto">
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
</div>

<!-- Whichever dialog is open (see $lib/dialogs.svelte.ts) renders here, above everything. -->
<DialogHost />
<!-- Confirmations (see $lib/confirm.svelte.ts) open above any dialog. -->
<ConfirmHost />
