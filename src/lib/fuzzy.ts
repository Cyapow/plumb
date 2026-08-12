// Tiny fuzzy subsequence scorer for the command palette.
// Returns a score (higher = better) or null when the query doesn't match.
export function fuzzyScore(query: string, text: string): number | null {
  if (!query) return 0;
  const q = query.toLowerCase();
  const t = text.toLowerCase();

  // Fast path: contiguous substring is always a strong match.
  const idx = t.indexOf(q);
  if (idx !== -1) {
    let s = 100 - idx; // earlier is better
    if (idx === 0 || /[\s/\-_.]/.test(t[idx - 1])) s += 15; // word-boundary start
    return s - t.length * 0.02;
  }

  // Subsequence match with bonuses for consecutive / word-start hits.
  let qi = 0;
  let score = 0;
  let last = -2;
  for (let ti = 0; ti < t.length && qi < q.length; ti++) {
    if (t[ti] === q[qi]) {
      score += last === ti - 1 ? 4 : 1;
      if (ti === 0 || /[\s/\-_.]/.test(t[ti - 1])) score += 3;
      last = ti;
      qi++;
    }
  }
  if (qi < q.length) return null;
  return score - t.length * 0.02;
}
