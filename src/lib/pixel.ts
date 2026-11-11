// Generated pixel art: an 8×8 "block" texture per instance.
// Deterministic from a seed string, so the same instance always gets the same block.

export type Pixel = { x: number; y: number; fill: string };

export function rng(seed: string) {
  let h = 1779033703 ^ seed.length;
  for (let i = 0; i < seed.length; i++) {
    h = Math.imul(h ^ seed.charCodeAt(i), 3432918353);
    h = (h << 13) | (h >>> 19);
  }
  return () => {
    h = Math.imul(h ^ (h >>> 16), 2246822507);
    h = Math.imul(h ^ (h >>> 13), 3266489909);
    return ((h ^= h >>> 16) >>> 0) / 4294967296;
  };
}

type Block = { top?: string[]; body: string[]; spot?: string[] };

const STONE = ["#7c7c7c", "#898989", "#6d6d6d", "#939393"];

export const BLOCKS = {
  grass: {
    top: ["#5c9a37", "#6aae42", "#4e8a2e", "#78bf4b"],
    body: ["#79553a", "#8a6243", "#6a4a32", "#93694a"],
  },
  dirt: { body: ["#79553a", "#8a6243", "#6a4a32", "#93694a"] },
  sand: { body: ["#dbcb93", "#e4d6a2", "#cfbd83", "#c7b479"] },
  copper: {
    body: ["#c26c47", "#d27b54", "#a95b3b", "#b8643f"],
    spot: ["#4fa38f", "#5db59f"],
  },
  stone: { body: STONE },
  nether: {
    body: ["#6c2a2a", "#7b3131", "#5a2222", "#8c3a3a"],
    spot: ["#9b4545"],
  },
  ore: { body: STONE, spot: ["#5ad5cf", "#7be6e1", "#3fb5b0"] },
  redstone: { body: STONE, spot: ["#c41e1e", "#e33232", "#9c1414"] },
  purpur: { body: ["#a67ba6", "#b38ab3", "#946894", "#c09ac0"] },
  moss: { body: ["#5a7a33", "#678a3b", "#4c6a2b", "#73974a"] },
  ice: { body: ["#8fb5f5", "#9fc1f7", "#7ea6ec", "#b3cff9"] },
} satisfies Record<string, Block>;

export type BlockKind = keyof typeof BLOCKS;
const KINDS = (Object.keys(BLOCKS) as BlockKind[]).filter((k) => k !== "dirt");

/** Picks a block for something that doesn't have one chosen (e.g. an instance id). */
export function kindFor(seed: string): BlockKind {
  return KINDS[Math.floor(rng(seed)() * KINDS.length)];
}

export function blockPixels(kind: BlockKind, seed: string): Pixel[] {
  const b: Block = BLOCKS[kind];
  const r = rng(seed + kind);
  const pick = (a: string[]) => a[Math.floor(r() * a.length)];
  const spots = new Set<string>();
  if (b.spot) {
    for (let c = 0; c < 3; c++) {
      const cx = 1 + Math.floor(r() * 6);
      const cy = 1 + Math.floor(r() * 6);
      spots.add(`${cx},${cy}`);
      spots.add(`${cx + 1},${cy}`);
      if (r() > 0.4) spots.add(`${cx},${cy + 1}`);
    }
  }
  const px: Pixel[] = [];
  for (let y = 0; y < 8; y++) {
    for (let x = 0; x < 8; x++) {
      let fill: string;
      if (b.top && (y < 2 || (y === 2 && r() > 0.45))) fill = pick(b.top);
      else if (b.spot && spots.has(`${x},${y}`)) fill = pick(b.spot);
      else fill = pick(b.body);
      px.push({ x, y, fill });
    }
  }
  return px;
}

/**
 * A wall of blocks for cover images: `cols × rows` blocks, each slightly different.
 * Grass only shows its green top on the first row, like a hillside.
 */
export function wallPixels(
  kind: BlockKind,
  seed: string,
  cols = 5,
  rows = 3,
): Pixel[] {
  const px: Pixel[] = [];
  for (let by = 0; by < rows; by++) {
    for (let bx = 0; bx < cols; bx++) {
      const k: BlockKind = kind === "grass" && by > 0 ? "dirt" : kind;
      for (const p of blockPixels(k, `${seed}-${bx}-${by}`)) {
        px.push({ x: p.x + bx * 8, y: p.y + by * 8, fill: p.fill });
      }
    }
  }
  return px;
}

/** What an instance's stand-in icon grows from: a shared pack's own id, so the pack looks the same for everyone in it. */
export const iconSeed = (i: {
  id: string;
  sync: { sharedInstanceId: string } | null;
}) => i.sync?.sharedInstanceId ?? i.id;

// Banner colours: Minecraft's dyes, toned down for the dark UI.
const DYES = [
  "#b0413e",
  "#c8742f",
  "#c9a33a",
  "#6a9a3a",
  "#3e7a4a",
  "#2f8a8f",
  "#4c86c4",
  "#3d4f9a",
  "#7a4aa0",
  "#a84a8c",
  "#d7d9dc",
  "#2a2e35",
];

// Banner patterns on the 8×8 grid: true where the pattern colour goes.
const PATTERNS: ((x: number, y: number) => boolean)[] = [
  (x) => x > 2 && x < 5, // stripe down
  (_, y) => y > 2 && y < 5, // stripe across
  (x, y) => (x > 2 && x < 5) || (y > 2 && y < 5), // cross
  (x, y) => y >= 1.5 + Math.abs(x - 3.5) && y <= 3.5 + Math.abs(x - 3.5), // chevron
  (x, y) => !x || !y || x === 7 || y === 7, // border
  (x, y) => x > y, // half, corner to corner
  (x, y) => ((x >> 1) + (y >> 1)) % 2 === 0, // checks
  (x, y) => (x - 3.5) ** 2 + (y - 3.5) ** 2 < 6, // circle
  (_, y) => y < 3, // top band
  (x, y) => Math.abs(x - 3.5) + Math.abs(y - 3.5) < 3.6, // diamond
  (x, y) => Math.abs(x - y) < 2, // diagonal
];

/** How light a colour looks, 0 to 1. */
const lightness = (hex: string) => {
  const n = parseInt(hex.slice(1), 16);
  return (0.3 * (n >> 16) + 0.59 * ((n >> 8) & 255) + 0.11 * (n & 255)) / 255;
};

const shade = (hex: string, f: number) => {
  const n = parseInt(hex.slice(1), 16);
  const c = [n >> 16, (n >> 8) & 255, n & 255].map((v) =>
    Math.min(255, Math.round(v * f)),
  );
  return `rgb(${c.join(",")})`;
};

/** A group's banner: a dye colour and a pattern in a second colour that stands out from it. */
export function bannerPixels(seed: string): Pixel[] {
  const r = rng(seed);
  // Black never as the background: on Atlas's dark panels the tile's edges would vanish.
  const bases = DYES.filter((c) => lightness(c) > 0.2);
  const base = bases[Math.floor(r() * bases.length)];
  const others = DYES.filter(
    (c) => Math.abs(lightness(c) - lightness(base)) > 0.28,
  );
  const fg = others[Math.floor(r() * others.length)];
  const on = PATTERNS[Math.floor(r() * PATTERNS.length)];
  const px: Pixel[] = [];
  for (let y = 0; y < 8; y++) {
    for (let x = 0; x < 8; x++) {
      px.push({ x, y, fill: shade(on(x, y) ? fg : base, 0.93 + r() * 0.14) });
    }
  }
  return px;
}
