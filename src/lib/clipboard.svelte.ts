/**
 * Copy to the clipboard with a short-lived "Copied" state for the button.
 *
 *   const clip = copier();
 *   <button onclick={() => clip.copy(link, "link")}>{clip.copied === "link" ? "Copied" : "Copy link"}</button>
 *
 * `key` lets several buttons share one copier: only the one you clicked says "Copied".
 * Copying can fail (permissions, some webviews), so `failed` says which key didn't make it —
 * show the text for the user to copy by hand instead of silently doing nothing.
 * In the Tauri app, @tauri-apps/plugin-clipboard-manager is the most reliable option.
 *
 * Call during component setup (it registers a cleanup so no timer outlives the component).
 */
export function copier(ms = 1600) {
  let copied = $state<string | null>(null);
  let failed = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  // Returning a function from $effect = cleanup when the component is destroyed.
  $effect(() => () => clearTimeout(timer));

  return {
    get copied() {
      return copied;
    },
    get failed() {
      return failed;
    },
    /** Resolves to true if it worked. Never throws. */
    async copy(text: string, key = text): Promise<boolean> {
      clearTimeout(timer);
      try {
        await navigator.clipboard.writeText(text);
        copied = key;
        failed = null;
        timer = setTimeout(() => (copied = null), ms);
        return true;
      } catch {
        copied = null;
        failed = key;
        timer = setTimeout(() => (failed = null), ms * 3);
        return false;
      }
    },
  };
}
