<script lang="ts" module>
  import { blockPixels, kindFor, rng } from "$lib/pixel";

  type RGB = [number, number, number];
  const BG: RGB = [24, 29, 36]; // --bg-raise
  const hex = (h: string): RGB =>
    [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)) as RGB;
  const lum = ([r, g, b]: RGB) => 0.2126 * r + 0.7152 * g + 0.0722 * b;
  const d2 = (p: RGB, q: RGB) =>
    (p[0] - q[0]) ** 2 + (p[1] - q[1]) ** 2 + (p[2] - q[2]) ** 2;
  const mix = (a: RGB, b: RGB, t: number) =>
    a.map((v, i) => v + (b[i] - v) * t) as RGB;
  const BAYER = [
    [0, 8, 2, 10],
    [12, 4, 14, 6],
    [3, 11, 1, 9],
    [15, 7, 13, 5],
  ];

  /**
   * A picture's main colours, dark to light: a small k-means over its pixels, small
   * clusters dropped, and black and white pulled toward the middle so neither takes over.
   */
  export function stops(px: RGB[], k = 6): RGB[] {
    const sorted = [...px].sort((a, b) => lum(a) - lum(b));
    let c = Array.from(
      { length: k },
      (_, i) => sorted[Math.floor(((i + 0.5) * sorted.length) / k)],
    );
    let groups: RGB[][] = [];
    for (let it = 0; it < 12; it++) {
      groups = c.map(() => []);
      for (const p of px) {
        let bi = 0;
        c.forEach((q, i) => d2(p, q) < d2(p, c[bi]) && (bi = i));
        groups[bi].push(p);
      }
      c = groups.map((g, i) =>
        g.length
          ? ([0, 1, 2].map(
              (j) => g.reduce((s, p) => s + p[j], 0) / g.length,
            ) as RGB)
          : c[i],
      );
    }
    const found = c
      .map((col, i) => ({ col, w: groups[i].length / px.length }))
      .filter((x) => x.w > 0.02);
    const big = found.filter((x) => x.w >= 0.06);
    return (big.length >= 2 ? big : found)
      .map((x) => x.col)
      .sort((a, b) => lum(a) - lum(b))
      .map((col) => {
        const l = lum(col);
        if (l < 50) return mix(col, [70, 76, 86], (50 - l) / 120);
        if (l > 200) return mix(col, [190, 196, 204], (l - 200) / 140);
        return col;
      });
  }

  /** The pixels of the generated block an icon falls back to (see BlockArt). */
  export const blockColours = (seed: string): RGB[] =>
    blockPixels(kindFor(seed), seed).map((p) => hex(p.fill));

  export const W = 155;
  export const H = 16;
  /**
   * The cover, pixel by pixel: 8 × 8 blocks of the colours dark to light, each textured
   * like a Minecraft block and dithered into the next where they meet, a little toward
   * the dialog's own colour so text and buttons sit well on it.
   */
  export function coverPixels(colours: RGB[], seed: string): RGB[][] {
    const s = colours.length ? colours : [BG];
    const r = rng(seed);
    const cols = Math.ceil(W / 8);
    const out: RGB[][] = Array.from({ length: H }, () => []);
    for (let bx = 0; bx < cols; bx++)
      for (let by = 0; by < H / 8; by++) {
        const t = Math.max(
          0,
          Math.min(
            s.length - 1,
            ((bx + by * 0.5) / (cols - 0.5)) * (s.length - 1),
          ),
        );
        const i = Math.floor(t);
        const f = t - i;
        const next = s[Math.min(s.length - 1, i + 1)];
        for (let x = 0; x < 8; x++)
          for (let y = 0; y < 8; y++) {
            const X = bx * 8 + x;
            const Y = by * 8 + y;
            if (X >= W) continue;
            const c = f * 1.1 > (BAYER[Y % 4][X % 4] + 0.5) / 16 ? next : s[i];
            const lit = 0.86 + r() * 0.22;
            out[Y][X] = mix(
              c.map((v) => Math.min(255, v * lit)) as RGB,
              BG,
              0.3,
            );
          }
      }
    return out;
  }
</script>

<script lang="ts">
  // A strip across the top of a dialog in the pack's own colours, as Minecraft-style
  // blocks: from its icon, or from the generated block it shows when it has none.
  let {
    seed,
    src = null,
    class: className = "h-16",
  }: { seed: string; src?: string | null; class?: string } = $props();

  let canvas = $state<HTMLCanvasElement>();
  let colours = $state<RGB[]>([]);

  $effect(() => {
    colours = stops(blockColours(seed));
    if (!src || !/^(https?:|data:image\/)/.test(src)) return;
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      try {
        const c = document.createElement("canvas");
        c.width = c.height = 32;
        const g = c.getContext("2d", { willReadFrequently: true })!;
        g.drawImage(img, 0, 0, 32, 32);
        const d = g.getImageData(0, 0, 32, 32).data;
        const px: RGB[] = [];
        for (let i = 0; i < d.length; i += 4)
          if (d[i + 3] > 160) px.push([d[i], d[i + 1], d[i + 2]]);
        if (px.length) colours = stops(px);
      } catch {
        // A picture from a server that doesn't allow reading it: keep the block's colours.
      }
    };
    img.src = src;
  });

  $effect(() => {
    const g = canvas?.getContext("2d");
    if (!g || !colours.length) return;
    coverPixels(colours, seed).forEach((row, y) =>
      row.forEach((c, x) => {
        g.fillStyle = `rgb(${c.map(Math.round).join(" ")})`;
        g.fillRect(x, y, 1, 1);
      }),
    );
  });
</script>

<canvas
  bind:this={canvas}
  width={W}
  height={H}
  class="block w-full shrink-0 bg-(--bg-raise-2) [image-rendering:pixelated] {className}"
  aria-hidden="true"
></canvas>
