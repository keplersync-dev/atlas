<script lang="ts" generics="T extends string">
  import Icon from "./Icon.svelte";

  // A dropdown in the app's own style; the native <select>'s list is drawn by
  // Windows and ignores the theme. Arrow keys move, Enter picks, Escape closes.
  let {
    value = $bindable(),
    options,
    label,
  }: {
    value: T;
    options: { value: T; label: string }[];
    /** For screen readers: what is being chosen. */
    label: string;
  } = $props();

  let open = $state(false);
  // The option the arrow keys are on while the list is open.
  let active = $state(0);
  let root: HTMLDivElement;
  let trigger: HTMLButtonElement;

  const current = $derived(options.find((o) => o.value === value));

  function show() {
    active = Math.max(
      0,
      options.findIndex((o) => o.value === value),
    );
    open = true;
  }

  function pick(v: T) {
    value = v;
    open = false;
    trigger.focus();
  }

  function onkeydown(e: KeyboardEvent) {
    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
        e.preventDefault();
        show();
      }
      return;
    }
    if (e.key === "ArrowDown") active = (active + 1) % options.length;
    else if (e.key === "ArrowUp")
      active = (active - 1 + options.length) % options.length;
    else if (e.key === "Enter" || e.key === " ") pick(options[active].value);
    else if (e.key === "Escape" || e.key === "Tab") open = false;
    else return;
    if (e.key !== "Tab") e.preventDefault();
  }

  function onWindowClick(e: MouseEvent) {
    if (open && !root.contains(e.target as Node)) open = false;
  }
</script>

<svelte:window onclick={onWindowClick} />

<div class="relative" bind:this={root}>
  <button
    bind:this={trigger}
    type="button"
    aria-label={label}
    aria-haspopup="listbox"
    aria-expanded={open}
    onclick={() => (open ? (open = false) : show())}
    {onkeydown}
    class="flex h-8 items-center gap-2 rounded-md border bg-(--bg-raise) pr-2 pl-2.5 text-[12.5px] text-(--text-muted) transition-colors hover:text-(--text-primary) focus-visible:outline-none
      {open
      ? 'border-(--accent)'
      : 'border-(--line-2) focus-visible:border-(--accent)'}"
  >
    {current?.label}
    <Icon
      name="chev"
      class="size-3.5 transition-transform {open ? '-rotate-90' : 'rotate-90'}"
    />
  </button>

  {#if open}
    <div
      role="listbox"
      aria-label={label}
      class="absolute top-full right-0 z-30 mt-1.5 min-w-full rounded-lg border border-(--line-2) bg-[#1b1f25] p-1.25 shadow-[0_16px_40px_#0009]"
    >
      {#each options as o, i (o.value)}
        <button
          type="button"
          role="option"
          aria-selected={o.value === value}
          tabindex="-1"
          onclick={() => pick(o.value)}
          onmouseenter={() => (active = i)}
          class="flex w-full items-center gap-2.5 rounded-[5px] px-2.5 py-1.75 text-left text-[12.5px] whitespace-nowrap
            {i === active ? 'bg-(--bg-hover)' : ''}
            {o.value === value
            ? 'text-(--text-primary)'
            : 'text-(--text-muted)'}"
        >
          <span class="flex-1">{o.label}</span>
          <Icon
            name="check"
            class="size-3.5 text-(--accent) {o.value === value
              ? ''
              : 'invisible'}"
          />
        </button>
      {/each}
    </div>
  {/if}
</div>
