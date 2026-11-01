import type { MenuItem } from "../components/MenuList.svelte";

// The one right-click menu: whatever was right-clicked hands over the same items its ⋯
// menu shows, and ContextMenuHost draws them at the pointer.
const state = $state<{
  open: { x: number; y: number; items: MenuItem[] } | null;
}>({
  open: null,
});

export const contextMenu = {
  get open() {
    return state.open;
  },
};

/** Use as `oncontextmenu={(e) => openContextMenu(e, items)}`. */
export function openContextMenu(e: MouseEvent, items: MenuItem[]) {
  if (!items.some((item) => item !== "divider")) return;
  e.preventDefault();
  // The innermost thing wins: a member row inside a group panel opens the member's menu.
  e.stopPropagation();
  state.open = { x: e.clientX, y: e.clientY, items };
}

export function closeContextMenu() {
  state.open = null;
}
