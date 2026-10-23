<script lang="ts">
  import { confirmation, type Choice } from "$lib/confirm.svelte";
  import Button from "./Button.svelte";
  import Dialog from "./Dialog.svelte";

  // Rendered once in the app shell, after DialogHost, so it opens on top of any dialog.
  const open = $derived(confirmation.open);

  let typed = $state("");
  const ready = $derived(
    !open?.typeToConfirm || typed.trim() === open.typeToConfirm,
  );

  function answer(choice: Choice | null) {
    typed = "";
    open?.answer(choice);
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (ready) answer("confirm");
  }
</script>

{#key open}
  {#if open}
    <Dialog title={open.title} onclose={() => answer(null)} onsubmit={submit}>
      {#if open.message}
        <p class="text-[13px] text-(--text-muted)">{open.message}</p>
      {/if}
      {#if open.typeToConfirm}
        <label class="mt-3.5 block text-[12.5px]">
          <span class="text-(--text-muted)">
            Type <b class="font-mono text-(--text-primary)"
              >{open.typeToConfirm}</b
            > to confirm
          </span>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            bind:value={typed}
            autofocus
            autocomplete="off"
            spellcheck="false"
            class="mt-1.5 h-8 w-full rounded-md border border-(--line-2) bg-(--bg-raise-2) px-2.5 font-mono outline-none focus:border-(--accent)"
          />
        </label>
      {/if}

      {#snippet footer()}
        {#if open.alternativeLabel}
          <!-- The bigger step, apart from the usual one. -->
          <Button
            class="mr-auto text-(--danger)"
            disabled={!ready}
            onclick={() => answer("alternative")}
            >{open.alternativeLabel}</Button
          >
        {/if}
        <!-- Cancel is focused for actions that lose something, so Enter doesn't do them. -->
        <!-- svelte-ignore a11y_autofocus -->
        <Button
          variant="ghost"
          autofocus={open.danger && !open.typeToConfirm}
          onclick={() => answer(null)}>Cancel</Button
        >
        <!-- svelte-ignore a11y_autofocus -->
        <Button
          type="submit"
          variant={open.danger ? "danger" : "primary"}
          disabled={!ready}
          autofocus={!open.danger && !open.typeToConfirm}
          >{open.confirmLabel}</Button
        >
      {/snippet}
    </Dialog>
  {/if}
{/key}
