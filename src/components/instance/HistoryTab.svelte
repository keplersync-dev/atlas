<script lang="ts">
  import { api } from "$lib/api";
  import { fileSize, plural, timeAgo } from "$lib/format";
  import { resource } from "$lib/resource.svelte";
  import type { Backup, InstanceDetail } from "$lib/types";
  import Button from "../Button.svelte";

  let {
    instance,
    lockedReason,
    onrestored,
  }: {
    instance: InstanceDetail;
    lockedReason: string | null;
    onrestored: () => void;
  } = $props();

  const sharedId = $derived(instance.sync?.sharedInstanceId ?? null);
  // No shared instance → resolve to null without a request.
  const shared = resource(() =>
    sharedId ? api.getSharedInstance(sharedId) : Promise.resolve(null),
  );

  let restoring = $state<string | null>(null);
  let error = $state<string | null>(null);

  function backupLabel(b: Backup) {
    const versions =
      b.fromRevision != null && b.toRevision != null
        ? ` v${b.fromRevision} → v${b.toRevision}`
        : "";
    if (b.reason === "before_update") return `Before updating${versions}`;
    if (b.reason === "before_restore") return "Before a restore";
    return "Before reverting local changes";
  }

  async function restore(b: Backup) {
    if (
      !confirm(
        `Restore the files from “${backupLabel(b)}”? Atlas backs up the current files first.`,
      )
    )
      return;
    restoring = b.id;
    error = null;
    try {
      await api.restoreBackup(instance.id, b.id);
      onrestored();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      restoring = null;
    }
  }
</script>

{#if sharedId}
  <h2 class="mt-5 mb-3 text-[13px] font-semibold text-(--text-muted)">
    Versions
  </h2>
  {#if shared.loading && !shared.data}
    <p class="text-(--text-muted)">Loading history…</p>
  {:else if shared.error}
    <p class="text-(--danger)">{shared.error}</p>
  {:else if shared.data}
    <ol>
      {#each shared.data.revisions as rev, i (rev.revision)}
        {@const current = rev.revision === instance.sync?.syncedRevision}
        <li class="relative grid grid-cols-[56px_minmax(0,1fr)] gap-3.5 pb-5.5">
          <!-- the vertical line joining versions; not after the last one -->
          {#if i < shared.data.revisions.length - 1}
            <span class="absolute top-6.5 bottom-0 left-7 w-px bg-(--line-2)"
            ></span>
          {/if}
          <span
            class="relative grid h-6 place-items-center rounded-full font-mono text-[11.5px] font-semibold
              {current
              ? 'bg-(--accent)/12 text-(--accent)'
              : 'bg-(--bg-raise-2) text-(--text-muted)'}"
          >
            v{rev.revision}
          </span>
          <div>
            <b class="font-semibold">{rev.changelog ?? "No description"}</b>
            <span class="ml-2 text-xs text-(--text-faint)">
              {rev.createdBy?.displayName ?? "Deleted user"} · {timeAgo(
                rev.createdAt,
              )}{current ? " · your copy" : ""}
            </span>
            <div
              class="mt-1.5 flex flex-wrap gap-1.5 text-[11.5px] text-(--text-muted)"
            >
              {#each rev.changes.added as name (name)}
                <span class="rounded bg-(--bg-raise) px-2 py-0.5"
                  ><span class="text-(--online)">+</span> {name}</span
                >
              {/each}
              {#each rev.changes.updated as u (u.name)}
                <span class="rounded bg-(--bg-raise) px-2 py-0.5"
                  ><span class="text-(--accent)">↑</span> {u.name} {u.to}</span
                >
              {/each}
              {#each rev.changes.removed as name (name)}
                <span class="rounded bg-(--bg-raise) px-2 py-0.5"
                  ><span class="text-(--danger)">−</span> {name}</span
                >
              {/each}
            </div>
          </div>
        </li>
      {/each}
    </ol>
  {/if}
{/if}

<h2 class="mt-5 mb-1 text-[13px] font-semibold text-(--text-muted)">Backups</h2>
<p class="mb-2 text-xs text-(--text-faint)">
  Atlas copies files it's about to change. The last 3 are kept.
</p>
{#if error}<p class="mb-2 text-xs text-(--danger)" role="alert">{error}</p>{/if}
{#each instance.backups as b (b.id)}
  <div
    class="flex items-center gap-3.5 rounded-md px-2 py-2 hover:bg-(--bg-hover)"
  >
    <div class="min-w-0 flex-1">
      <b class="font-medium">{backupLabel(b)}</b>
      <small class="block text-[11.5px] text-(--text-faint)">
        {timeAgo(b.createdAt)} · {plural(b.fileCount, "file")} · {fileSize(
          b.sizeBytes,
        )}
      </small>
    </div>
    <Button
      size="sm"
      disabled={!!lockedReason || restoring !== null}
      onclick={() => restore(b)}
    >
      {restoring === b.id ? "Restoring…" : "Restore"}
    </Button>
  </div>
{:else}
  <p class="py-3 text-[12.5px] text-(--text-muted)">
    No backups yet. One is made before every update.
  </p>
{/each}
