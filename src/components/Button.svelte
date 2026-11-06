<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import Icon from "./Icon.svelte";

  type Variant = "default" | "primary" | "warn" | "danger" | "ghost";

  // `icon` goes before the label: an Icon name, or a snippet for anything else.
  let {
    variant = "default",
    size = "md",
    class: className = "",
    icon,
    children,
    ...rest
  }: HTMLButtonAttributes & {
    variant?: Variant;
    size?: "sm" | "md" | "lg";
    icon?: string | Snippet;
    children: Snippet;
  } = $props();

  // Full class strings only: Tailwind can't see classes built with `bg-${x}`.
  const variants: Record<Variant, string> = {
    default:
      "border-(--line-2) bg-(--bg-raise-2) hover:border-(--text-faint) hover:bg-(--bg-hover)",
    primary:
      "border-(--accent) bg-(--accent) font-semibold text-(--accent-ink) hover:border-(--accent-hover) hover:bg-(--accent-hover)",
    warn: "border-(--warn) bg-(--warn) font-semibold text-(--warn-ink) hover:bg-(--warn-hover)",
    danger:
      "border-(--danger) bg-(--danger) font-semibold text-(--danger-ink) hover:border-(--danger-hover) hover:bg-(--danger-hover)",
    ghost:
      "border-transparent bg-transparent text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)",
  };
  const sizes = {
    sm: "h-7 px-2.5 text-[13px]",
    md: "h-8 px-3.5",
    lg: "h-9.5 px-4.5 text-[14px]",
  };
</script>

<button
  type="button"
  {...rest}
  class="inline-flex items-center justify-center gap-1.5 rounded-md border font-medium whitespace-nowrap transition-colors
    disabled:cursor-not-allowed disabled:opacity-45 {variants[variant]} {sizes[
    size
  ]} {className}"
>
  {#if typeof icon === "string"}
    <!-- The icons' own padding leaves more room on the left; even it out. -->
    <Icon name={icon} class="-ml-0.5 size-3.5" />
  {:else if icon}
    {@render icon()}
  {/if}
  <!-- Trimmed to cap height and baseline, so the letters themselves are centred:
       Segoe's line box puts them about a pixel and a half low. -->
  <span class="[text-box:trim-both_cap_alphabetic]">{@render children()}</span>
</button>
