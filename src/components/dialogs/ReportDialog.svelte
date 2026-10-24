<script lang="ts">
  import { api } from "$lib/api";
  import type { ReportAbout } from "$lib/dialogs.svelte";
  import type { ReportReason } from "$lib/types";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";

  let { about, onclose }: { about: ReportAbout; onclose: () => void } =
    $props();

  // The reasons that make sense for what's reported.
  const REASONS: Record<ReportAbout["kind"], ReportReason[]> = {
    file: ["copyright", "malware", "other"],
    shared_instance: ["copyright", "malware", "abuse", "other"],
    user: ["abuse", "other"],
  };
  const LABELS: Record<ReportReason, { title: string; hint: string }> = {
    copyright: {
      title: "It's someone else's work",
      hint: "Shared without the permission of whoever made it.",
    },
    malware: {
      title: "It's harmful",
      hint: "Malware, or something meant to damage a PC or an account.",
    },
    abuse: {
      title: "Harassment or abuse",
      hint: "Hateful, threatening or meant to upset someone.",
    },
    other: { title: "Something else", hint: "Say what in the details." },
  };
  const WHAT: Record<ReportAbout["kind"], string> = {
    file: "file",
    shared_instance: "shared instance",
    user: "person",
  };

  let reason = $state<ReportReason | null>(null);
  let details = $state("");
  let sending = $state(false);
  let sent = $state(false);
  let error = $state<string | null>(null);

  async function send(e: SubmitEvent) {
    e.preventDefault();
    if (!reason) return;
    sending = true;
    error = null;
    const text = details.trim() || null;
    try {
      if (about.kind === "file") {
        await api.reportFile(about.instanceId, about.contentId, reason, text);
      } else {
        await api.report({
          targetType: about.kind,
          targetId: about.id,
          reason,
          details: text,
        });
      }
      sent = true;
    } catch (err) {
      error = (err as Error).message;
    } finally {
      sending = false;
    }
  }
</script>

<Dialog
  title="Report {about.name}"
  description={sent
    ? undefined
    : `Tell Atlas what's wrong with this ${WHAT[about.kind]}. Reports are private.`}
  {onclose}
  onsubmit={send}
>
  {#if sent}
    <p class="text-[13px] text-(--text-muted)">
      Thanks. Atlas will look into it, and remove it if it breaks the rules.
    </p>
  {:else}
    <div class="grid gap-1.5" role="radiogroup" aria-label="Why">
      {#each REASONS[about.kind] as r (r)}
        <label
          class="flex cursor-pointer items-start gap-2.5 rounded-md border px-3 py-2 transition-colors
            {reason === r
            ? 'border-(--accent) bg-(--accent)/8'
            : 'border-(--line-2) hover:bg-(--bg-hover)'}"
        >
          <input
            type="radio"
            name="reason"
            value={r}
            bind:group={reason}
            class="mt-1 accent-(--accent)"
          />
          <span>
            <b class="block font-medium">{LABELS[r].title}</b>
            <small class="text-[11.5px] text-(--text-faint)"
              >{LABELS[r].hint}</small
            >
          </span>
        </label>
      {/each}
    </div>
    <h3 class="mt-4 mb-1.5 text-xs font-semibold text-(--text-muted)">
      Details <span class="font-normal text-(--text-faint)">(optional)</span>
    </h3>
    <textarea
      bind:value={details}
      maxlength="2000"
      rows="3"
      placeholder={about.kind === "file" || reason === "copyright"
        ? "If it's your work, say where it's from"
        : "Anything that helps"}
      class="w-full resize-none rounded-md border border-(--line-2) bg-(--bg) px-3 py-2 leading-snug outline-none focus:border-(--accent)"
    ></textarea>
  {/if}

  {#snippet footer()}
    {#if error}
      <span class="mr-auto text-xs text-(--danger)" role="alert">{error}</span>
    {/if}
    {#if sent}
      <Button variant="primary" onclick={onclose}>Done</Button>
    {:else}
      <Button variant="ghost" onclick={onclose}>Cancel</Button>
      <Button type="submit" variant="primary" disabled={!reason || sending}>
        {sending ? "Sending…" : "Report"}
      </Button>
    {/if}
  {/snippet}
</Dialog>
