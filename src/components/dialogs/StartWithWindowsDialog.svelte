<script lang="ts">
  import { api } from "$lib/api";
  import { showError } from "$lib/toasts.svelte";
  import Button from "../Button.svelte";
  import Dialog from "../Dialog.svelte";
  import Icon from "../Icon.svelte";
  import Switch from "../Switch.svelte";

  // Shown once, after the first accepted pack: the picture says why, the switch decides.
  let { owner, onclose }: { owner: string | null; onclose: () => void } =
    $props();

  let on = $state(true);
  let saving = $state(false);

  const STEPS = $derived([
    { icon: "users", text: `${owner ?? "Your friend"} sends an update` },
    { icon: "download", text: "Your pack updates in the background" },
    { icon: "check", text: "You press Play and you're ready" },
  ]);

  /** Closing it any way counts as seen; only Done turns starting with Windows on. */
  async function save(start: boolean) {
    saving = true;
    try {
      await api.updateAppSettings(
        start
          ? {
              startWithWindowsOffered: true,
              startWithWindows: true,
              closeToTray: true,
            }
          : { startWithWindowsOffered: true },
      );
    } catch (err) {
      showError((err as Error).message);
    }
    onclose();
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!saving) save(on);
  }
</script>

<Dialog
  title="Keep Atlas running"
  onclose={() => !saving && save(false)}
  onsubmit={submit}
>
  <ol class="my-2 grid grid-cols-[1fr_auto_1fr_auto_1fr] items-center gap-2">
    {#each STEPS as step, i (step.icon)}
      {#if i}
        <li aria-hidden="true" class="text-(--text-faint)">
          <Icon name="chev" class="size-4" />
        </li>
      {/if}
      <li
        class="flex h-full flex-col items-center gap-2 rounded-lg bg-(--bg-raise) px-2 py-3 text-center text-[13px]"
      >
        <span
          class="grid size-9 place-items-center rounded-full bg-(--accent)/15 text-(--accent)"
        >
          <Icon name={step.icon} class="size-4.5" />
        </span>
        {step.text}
      </li>
    {/each}
  </ol>

  <div
    class="mt-4 flex items-center gap-3 rounded-lg bg-(--bg-raise) px-3.5 py-3"
  >
    <div class="min-w-0 flex-1">
      <b class="block font-medium">Start Atlas with Windows</b>
      <small class="block text-[13px] text-(--text-muted)">
        It waits in the tray, so updates are in before you play.
      </small>
    </div>
    <Switch
      checked={on}
      label="Start Atlas with Windows"
      onchange={(next) => (on = next)}
    />
  </div>

  {#snippet footer()}
    <Button type="submit" variant="primary" disabled={saving}>Done</Button>
  {/snippet}
</Dialog>
