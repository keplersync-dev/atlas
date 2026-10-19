<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  type Variant = "default" | "primary" | "warn" | "ghost";

  // `children` is whatever is written between <Button> and </Button>.
  // `...rest` collects every other attribute (onclick, disabled, aria-label…)
  // so the parent can use <Button> exactly like a normal <button>.
  let {
    variant = "default",
    size = "md",
    class: className = "",
    children,
    ...rest
  }: HTMLButtonAttributes & {
    variant?: Variant;
    size?: "sm" | "md" | "lg";
    children: Snippet;
  } = $props();

  // Full class strings only: Tailwind can't see classes built with `bg-${x}`.
  const variants: Record<Variant, string> = {
    default:
      "border-(--line-2) bg-(--bg-raise-2) hover:border-[#3c4450] hover:bg-[#252b33]",
    primary:
      "border-(--accent) bg-(--accent) font-semibold text-(--accent-ink) hover:border-[#a6e5f3] hover:bg-[#a6e5f3]",
    warn: "border-(--warn) bg-(--warn) font-semibold text-[#2e1f05] hover:bg-[#f2c983]",
    ghost:
      "border-transparent bg-transparent text-(--text-muted) hover:bg-(--bg-hover) hover:text-(--text-primary)",
  };
  const sizes = {
    sm: "h-7 px-2.5 text-xs",
    md: "h-8 px-3.5",
    lg: "h-9.5 px-4.5 text-[13.5px]",
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
  {@render children()}
</button>
