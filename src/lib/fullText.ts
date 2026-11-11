/** Hovering cut-off (truncated) text shows it in full; text that fits gets no hover. */
export function fullText(node: HTMLElement) {
  const check = () => {
    node.title =
      node.scrollWidth > node.clientWidth
        ? (node.textContent ?? "").replace(/\s+/g, " ").trim()
        : "";
  };
  node.addEventListener("pointerenter", check);
  return { destroy: () => node.removeEventListener("pointerenter", check) };
}
