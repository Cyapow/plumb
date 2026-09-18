// Word-level diff between a removed line and its paired added line, so the UI
// can highlight exactly which words changed (not just the whole line).

export interface Seg {
  text: string;
  changed: boolean;
}

function tokenize(s: string): string[] {
  return s.match(/(\s+|\w+|[^\w\s])/g) ?? [];
}

function merge(tokens: string[], flags: boolean[]): Seg[] {
  const segs: Seg[] = [];
  tokens.forEach((t, i) => {
    const changed = flags[i];
    const last = segs[segs.length - 1];
    if (last && last.changed === changed) last.text += t;
    else segs.push({ text: t, changed });
  });
  return segs;
}

/** Token-pair budget for the LCS table. Above this the line is treated as
 *  wholly changed — a minified line can tokenise to tens of thousands of
 *  tokens, and an n×m table of that size is an out-of-memory crash. */
const MAX_CELLS = 250_000;

/** Returns [removed-line segments, added-line segments]. */
export function wordDiff(a: string, b: string): [Seg[], Seg[]] {
  const at = tokenize(a);
  const bt = tokenize(b);
  const n = at.length;
  const m = bt.length;

  if (n === 0 || m === 0 || n * m > MAX_CELLS) {
    const whole = (s: string): Seg[] => (s ? [{ text: s, changed: true }] : []);
    return [whole(a), whole(b)];
  }

  // LCS length table.
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] = at[i] === bt[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  const aFlags = new Array(n).fill(true);
  const bFlags = new Array(m).fill(true);
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (at[i] === bt[j]) {
      aFlags[i] = false;
      bFlags[j] = false;
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      i++;
    } else {
      j++;
    }
  }
  return [merge(at, aFlags), merge(bt, bFlags)];
}
