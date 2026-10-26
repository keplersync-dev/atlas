<script lang="ts">
  import { social } from "$lib/stores";
  import type { AuthProvider } from "$lib/types";
  import Button from "./Button.svelte";
  import ProviderIcon from "./ProviderIcon.svelte";

  const label = (provider: AuthProvider, name: string) =>
    social.signingIn === provider ? "Start again" : `Sign in with ${name}`;
</script>

{#if !social.hasServer}
  <p class="mt-4 text-[12px] text-(--text-faint)">
    Friends and sharing aren't available in this copy of Atlas.
  </p>
{:else}
  <div class="mt-4 flex justify-center gap-2">
    <Button
      variant="primary"
      class="gap-2"
      onclick={() => social.signIn("minecraft")}
    >
      <ProviderIcon provider="minecraft" class="size-4" />
      {label("minecraft", "Minecraft")}
    </Button>
    <Button class="gap-2" onclick={() => social.signIn("discord")}>
      <ProviderIcon provider="discord" class="size-4" />
      {label("discord", "Discord")}
    </Button>
  </div>
  {#if social.signingIn}
    <p class="mt-2 text-[12px] text-(--text-faint)">
      Finish signing in in your browser.
      <button
        onclick={() => social.cancelSignIn()}
        class="underline hover:text-(--text-primary)">Cancel</button
      >
    </p>
  {:else if social.signInError}
    <p class="mt-2 text-[12px] text-(--danger)" role="alert">
      {social.signInError}
    </p>
  {/if}
{/if}
