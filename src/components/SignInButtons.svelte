<script lang="ts">
  import { social } from "$lib/stores";
  import Button from "./Button.svelte";
  import ProviderIcon from "./ProviderIcon.svelte";

  /** Left-aligned, for use inside a dialog's text; centred on empty pages. */
  let { start = false }: { start?: boolean } = $props();
</script>

{#if !social.hasServer}
  <p class="mt-4 text-[13px] text-(--text-faint)">
    Friends and sharing aren't available in this copy of Atlas.
  </p>
{:else}
  <div class="mt-4 flex gap-2 {start ? '' : 'justify-center'}">
    <Button variant="primary" class="gap-2" onclick={() => social.signIn()}>
      {#snippet icon()}<ProviderIcon class="size-4" />{/snippet}
      {social.signingIn ? "Start again" : "Sign in with Minecraft"}
    </Button>
  </div>
  {#if social.signingIn}
    <p class="mt-2 text-[13px] text-(--text-faint)">
      Finish signing in in your browser.
      <button
        onclick={() => social.cancelSignIn()}
        class="underline hover:text-(--text-primary)">Cancel</button
      >
    </p>
  {:else if social.signInError}
    <p class="mt-2 text-[13px] text-(--danger)" role="alert">
      {social.signInError}
    </p>
  {/if}
{/if}
