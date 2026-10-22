<script lang="ts">
  import { api } from "$lib/api";
  import { fileSize, plural, versionLabel } from "$lib/format";
  import { go } from "$lib/nav.svelte";
  import { resource } from "$lib/resource.svelte";
  import { instances, social } from "$lib/stores";
  import type { Id, InstanceSummary, Launcher } from "$lib/types";
  import BlockArt from "../BlockArt.svelte";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";
  import OptionalPicker from "../OptionalPicker.svelte";

  let {
    sharedInstanceId,
    onclose,
  }: { sharedInstanceId: Id; onclose: () => void } = $props();

  const invite = $derived(social.shared.find((s) => s.id === sharedInstanceId));
  const launchers = resource(() => api.listLaunchers());
  const creatable = $derived(
    launchers.data?.filter((l) => l.canCreateInstances) ?? [],
  );

  // Instances already tracked here that aren't shared: the files can go into one of those.
  const spare = $derived(
    instances.list.filter((i) => !i.sync && i.status !== "missing"),
  );

  // Where the files go: a new instance in a launcher, or an instance you already have.
  type Target = { kind: "launcher" | "instance"; id: Id };

  // A *writable* $derived: it starts as "the first launcher that can receive
  // instances" (or the first spare instance), and fills in by itself once the lists
  // load, but assigning to it overrides that — which is what clicking a radio does.
  let target = $derived<Target | null>(
    creatable[0]
      ? { kind: "launcher", id: creatable[0].id }
      : spare[0]
        ? { kind: "instance", id: spare[0].id }
        : null,
  );
  const picked = (kind: Target["kind"], id: Id) =>
    target?.kind === kind && target.id === id;

  // Optional extras the user ticks (none to begin with).
  let optional = $state<string[]>([]);

  let submitting = $state(false);
  let error = $state<string | null>(null);

  const canSubmit = $derived(!submitting && !!invite && !!target);

  function whyNot(l: Launcher) {
    return `Atlas can't create instances in ${l.name} yet. Make one there, track it, and pick it below.`;
  }

  /** A note when an existing instance is on other versions than the shared one. */
  function mismatch(i: InstanceSummary) {
    if (!invite) return null;
    const same =
      i.minecraftVersion === invite.minecraftVersion &&
      i.loader === invite.loader;
    return same
      ? null
      : `It's on ${versionLabel(i.loader, i.minecraftVersion)}; this needs ${versionLabel(invite.loader, invite.minecraftVersion)}`;
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!canSubmit || !target) return;
    submitting = true;
    error = null;
    try {
      const created =
        target.kind === "launcher"
          ? await api.acceptInvite(sharedInstanceId, target.id, optional)
          : await api.acceptInviteInto(sharedInstanceId, target.id, optional);
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
      A new instance
    </h3>

    {#if launchers.loading}
      <p class="text-[12.5px] text-(--text-muted)">Looking for launchers…</p>
    {:else if launchers.error}
      <p class="text-[12.5px] text-(--danger)">{launchers.error}</p>
    {:else if creatable.length === 0}
      <p class="text-[12.5px] text-(--text-muted)">
        Atlas can create instances in Prism Launcher and MultiMC. With another
        launcher, make an instance there, track it, and pick it below.
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
            name="target"
            value={l.id}
            checked={picked("launcher", l.id)}
            disabled={!ok}
            onchange={() => (target = { kind: "launcher", id: l.id })}
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

    {#if spare.length}
      <h3 class="mt-4 mb-1.5 text-xs font-semibold text-(--text-muted)">
        Or an instance you already have
      </h3>
      {#each spare as i (i.id)}
        {@const note = mismatch(i)}
        <label
          class="-mx-2 flex cursor-pointer items-center gap-3 rounded-md px-2 py-2 hover:bg-(--bg-hover)"
        >
          <input
            type="radio"
            name="target"
            value={i.id}
            checked={picked("instance", i.id)}
            onchange={() => (target = { kind: "instance", id: i.id })}
            class="accent-(--accent)"
          />
          <span class="min-w-0 flex-1">
            <b class="block truncate font-medium">{i.name}</b>
            <small
              class="block text-[11.5px] {note
                ? 'text-(--warn)'
                : 'text-(--text-faint)'}"
            >
              {note ??
                `${i.launcher.name} · the shared files are added, yours stay`}
            </small>
          </span>
        </label>
      {/each}
    {/if}

    <OptionalPicker items={invite.optionalItems} bind:selected={optional} />

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
