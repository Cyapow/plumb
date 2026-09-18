// Row shaping for the diff viewer, kept pure so it's testable without the DOM.
// Unified view renders hunks as a flat list of head/line rows; side-by-side
// pairs deletions with additions so the two panes share one row index space.
import type { DiffHunk, DiffLine } from "./git";

export interface Cell {
  l: DiffLine;
  li: number;
}
export interface SplitRow {
  left?: Cell;
  right?: Cell;
  ctx?: boolean;
}

export type UnifiedRow = { kind: "head"; hi: number } | { kind: "line"; hi: number; li: number; l: DiffLine };
export type SplitItem = { kind: "head"; hi: number } | { kind: "row"; hi: number; ri: number; row: SplitRow };

/** Side-by-side: pair deleted (left) with added (right) lines; context spans both. */
export function buildSplitRows(hunks: DiffHunk[]): SplitRow[][] {
  return hunks.map((h) => {
    const rows: SplitRow[] = [];
    const lines = h.lines;
    let i = 0;
    while (i < lines.length) {
      if (lines[i].origin === " ") {
        rows.push({ left: { l: lines[i], li: i }, right: { l: lines[i], li: i }, ctx: true });
        i++;
      } else {
        const dels: Cell[] = [];
        const adds: Cell[] = [];
        while (i < lines.length && lines[i].origin === "-") dels.push({ l: lines[i], li: i++ });
        while (i < lines.length && lines[i].origin === "+") adds.push({ l: lines[i], li: i++ });
        const n = Math.max(dels.length, adds.length);
        for (let k = 0; k < n; k++) rows.push({ left: dels[k], right: adds[k] });
      }
    }
    return rows;
  });
}

export function flattenUnified(hunks: DiffHunk[]): UnifiedRow[] {
  const out: UnifiedRow[] = [];
  hunks.forEach((h, hi) => {
    out.push({ kind: "head", hi });
    h.lines.forEach((l, li) => out.push({ kind: "line", hi, li, l }));
  });
  return out;
}

export function flattenSplit(hunks: DiffHunk[], splitRows: SplitRow[][]): SplitItem[] {
  const out: SplitItem[] = [];
  hunks.forEach((_, hi) => {
    out.push({ kind: "head", hi });
    (splitRows[hi] ?? []).forEach((row, ri) => out.push({ kind: "row", hi, ri, row }));
  });
  return out;
}
