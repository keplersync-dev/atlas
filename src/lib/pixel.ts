// Generated pixel art: an 8×8 "block" texture per instance and a Minecraft-style head per person.
// Deterministic from a seed string, so the same instance always gets the same block.

export type Pixel = { x: number; y: number; fill: string };

function rng(seed: string) {
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

const SKIN = ["#f1c7a5", "#d9a37c", "#b57b52", "#8a5a3a", "#5e3b26"];
const HAIR = [
  "#2b1d14",
  "#4a2f1b",
  "#7a4a22",
  "#c99a4b",
  "#1d1d22",
  "#b23a2a",
  "#e8e2d0",
];
const EYES = ["#3a5fcd", "#3d7a3a", "#5a3a22", "#2a2a2a"];

const darken = (hex: string) =>
  hex.replace(/[0-9a-f]{2}/gi, (h) =>
    Math.max(0, parseInt(h, 16) - 22)
      .toString(16)
      .padStart(2, "0"),
  );

/** A Minecraft-style face. Seed with the Minecraft username so it's stable. */
export function headPixels(seed: string): Pixel[] {
  const r = rng(seed);
  const skin = SKIN[Math.floor(r() * SKIN.length)];
  const hair = HAIR[Math.floor(r() * HAIR.length)];
  const eye = EYES[Math.floor(r() * EYES.length)];
  const fringe = Math.floor(r() * 3);
  const px: Pixel[] = [];
  for (let y = 0; y < 8; y++) {
    for (let x = 0; x < 8; x++) {
      let fill = skin;
      if (y < 2 || (y === 2 && (x < 1 + fringe || x > 6 - fringe || r() > 0.7)))
        fill = r() > 0.8 ? darken(hair) : hair;
      else if (y === 3 && (x === 0 || x === 7)) fill = hair;
      else if (y === 4 && (x === 1 || x === 6)) fill = "#f4f4f4";
      else if (y === 4 && (x === 2 || x === 5)) fill = eye;
      else if (y === 6 && (x === 3 || x === 4)) fill = darken(darken(skin));
      else if (r() > 0.85) fill = darken(skin);
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
