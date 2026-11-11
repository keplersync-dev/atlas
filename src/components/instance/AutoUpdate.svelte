<script lang="ts">
  import { api } from "$lib/api";
  import { nameList } from "$lib/format";
  import { instances, social } from "$lib/stores";
  import { showError } from "$lib/toasts.svelte";
  import type { Id, InstanceSyncInfo, SharedMember } from "$lib/types";
  import Switch from "../Switch.svelte";

  // Whether new versions of a shared pack install by themselves. The owner sends their
  // own, so they only get the switch while an editor can send one, and it names them.
  let {
    instanceId,
    sync,
    members,
    class: className = "",
  }: {
    instanceId: Id;
    sync: InstanceSyncInfo;
    /** The pack's members when the page has them fresh; else the shared list's. */
    members?: SharedMember[];
    class?: string;
  } = $props();

  const isOwner = $derived(sync.role === "owner");
  const editors = $derived(
    isOwner
      ? (
          members ??
          social.shared.find((s) => s.id === sync.sharedInstanceId)?.members ??
          []
        )
          .filter((m) => m.role === "editor" && m.status === "active")
          .map((m) => m.user.displayName)
      : [],
  );
  const label = $derived(
    !isOwner
      ? "Keep it up to date automatically"
      : editors.length === 1
        ? `Install ${editors[0]}'s updates automatically`
        : `Install updates from ${nameList(editors)} automatically`,
  );

  let saving = $state(false);
  async function set(on: boolean) {
    saving = true;
    try {
      await api.updateInstance(instanceId, { autoUpdate: on });
      await instances.load();
    } catch (e) {
      showError((e as Error).message);
    } finally {
      saving = false;
    }
  }
</script>

{#if !isOwner || editors.length}
  <div
    class="flex items-center gap-3 rounded-lg bg-(--bg-raise) px-3.5 py-2.5 {className}"
  >
    <div class="min-w-0 flex-1">
      <b class="block font-medium">{label}</b>
      <small class="block text-[13px] text-(--text-muted)">
        Before you next play. Atlas asks first if one would undo your changes or
        add an uploaded mod.
      </small>
    </div>
    <Switch
      checked={sync.autoUpdate}
      {label}
      disabled={saving}
      onchange={set}
    />
  </div>
{/if}
