<script lang="ts">
  import { api } from "$lib/api";
  import { fileSize, plural, versionLabel } from "$lib/format";
  import { go } from "$lib/nav.svelte";
  import { resource } from "$lib/resource.svelte";
  import { instances, social } from "$lib/stores";
  import type { Id, Launcher } from "$lib/types";
  import BlockArt from "../BlockArt.svelte";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";

  let {
    sharedInstanceId,
    onclose,
  }: { sharedInstanceId: Id; onclose: () => void } = $props();

  const invite = $derived(social.shared.find((s) => s.id === sharedInstanceId));
  const launchers = resource(() => api.listLaunchers());
  const creatable = $derived(
    launchers.data?.filter((l) => l.canCreateInstances) ?? [],
  );

  // A *writable* $derived: it starts as "the first launcher that can receive
  // instances" (and fills in by itself once the list loads), but assigning to it
  // overrides that — which is what clicking a radio button does.
  let launcherId = $derived(creatable[0]?.id ?? null);

  let submitting = $state(false);
  let error = $state<string | null>(null);

  const canSubmit = $derived(!submitting && !!invite && !!launcherId);

  function whyNot(l: Launcher) {
    return `Atlas can't create instances in ${l.name} yet. Track one you made there instead.`;
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!canSubmit || !launcherId) return;
    submitting = true;
    error = null;
    try {
      const created = await api.acceptInvite(sharedInstanceId, launcherId);
      await Promise.all([instances.load(), social.load()]);
      onclose();
      // Open the new instance: its sync bar shows the files arriving.
      go({ page: "instance", id: created.id });
    } catch (err) {
      error = (err as Error).message;
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog
  title={invite ? `Add ${invite.name}` : "Invite"}
  description={invite
    ? `${versionLabel(invite.loader, invite.minecraftVersion)} · ${plural(invite.contentCount, "item")} · ${fileSize(invite.sizeBytes)}`
    : undefined}
  {onclose}
  onsubmit={submit}
>
  {#snippet header()}
    <BlockArt seed={sharedInstanceId} class="size-10 rounded-md" />
  {/snippet}

  {#if !invite}
    <p class="py-4 text-[12.5px] text-(--text-muted)">
      This invite isn't available anymore.
    </p>
  {:else}
    <h3 class="mt-1 mb-1.5 text-xs font-semibold text-(--text-muted)">
      Which launcher should it go in?
    </h3>

    {#if launchers.loading}
      <p class="text-[12.5px] text-(--text-muted)">Looking for launchers…</p>
    {:else if launchers.error}
      <p class="text-[12.5px] text-(--danger)">{launchers.error}</p>
    {:else if creatable.length === 0}
      <p class="text-[12.5px] text-(--text-muted)">
        Atlas can create instances in Prism Launcher and the Modrinth App.
        Install one of them, then try again.
      </p>
    {:else}
      {#each launchers.data ?? [] as l (l.id)}
        {@const ok = l.canCreateInstances}
        <label
          class="-mx-2 flex items-center gap-3 rounded-md px-2 py-2 {ok
            ? 'cursor-pointer hover:bg-(--bg-hover)'
            : 'opacity-50'}"
        >
          <!-- Radios share a `name`; checked/onchange keep them in step with launcherId. -->
          <input
            type="radio"
            name="launcher"
            value={l.id}
            checked={launcherId === l.id}
            disabled={!ok}
            onchange={() => (launcherId = l.id)}
            class="accent-(--accent)"
          />
          <span class="min-w-0 flex-1">
            <b class="block font-medium">{l.name}</b>
            <small class="block text-[11.5px] text-(--text-faint)">
              {ok
                ? `Atlas creates a new instance there named “${invite.name}”`
                : whyNot(l)}
            </small>
          </span>
        </label>
      {/each}
    {/if}

    <p class="mt-3.5 text-xs text-(--text-faint)">
      Your launcher downloads Minecraft and {versionLabel(invite.loader, null)} the
      first time you play. Atlas only adds the mods and configs.
    </p>
  {/if}

  {#snippet footer()}
    {#if error}<span class="mr-auto text-xs text-(--danger)" role="alert"
        >{error}</span
      >{/if}
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button type="submit" variant="primary" disabled={!canSubmit}>
      {submitting ? "Adding…" : "Add instance"}
    </Button>
  {/snippet}
</Dialog>
