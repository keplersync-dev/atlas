// Search that forgives: "sf5" finds "SkyFactory 5", "atm sky" finds "All the Mods 10 To
// the Sky". Every word of the query has to appear in order (letters may be spread out);
// matches at word starts and in runs score higher, so the likeliest result comes first.

/** How well one word of a query matches `text` (both lower case), or null for no match. */
function wordScore(word: string, text: string): number | null {
  const at = text.indexOf(word);
  // A plain substring beats any spread-out match; earlier and at a word start is better.
  if (at >= 0) return 100 + (at === 0 || /\W/.test(text[at - 1]) ? 50 : 0) - at;
  let score = 0;
  let from = 0;
  let last = -2;
  for (const ch of word) {
    const i = text.indexOf(ch, from);
    if (i < 0) return null;
    score += i === last + 1 ? 5 : 1; // a run of letters
    if (i === 0 || /\W/.test(text[i - 1])) score += 8; // a word start
    last = i;
    from = i + 1;
  }
  return score - (last - word.length) / 10;
}

/** A score for `query` against any of `texts` (higher is better), or null when it doesn't match. */
export function fuzzy(query: string, ...texts: (string | null | undefined)[]) {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return 0;
  const hay = texts.filter(Boolean).map((t) => t!.toLowerCase());
  let total = 0;
  for (const word of words) {
    const best = Math.max(
      ...hay.map((t) => wordScore(word, t) ?? Number.NEGATIVE_INFINITY),
    );
    if (best === Number.NEGATIVE_INFINITY) return null;
    total += best;
  }
  return total;
}

/** The items that match `query`, best first; all of them, in order, for an empty query. */
export function fuzzyFilter<T>(
  items: T[],
  query: string,
  texts: (item: T) => (string | null | undefined)[],
): T[] {
  if (!query.trim()) return items;
  return items
    .map((item) => ({ item, score: fuzzy(query, ...texts(item)) }))
    .filter((x) => x.score !== null)
    .sort((a, b) => b.score! - a.score!)
    .map((x) => x.item);
}
