import { describe, it, expect } from "vitest";
import type { DiffHunk, DiffLine } from "./git";
import { buildSplitRows, flattenUnified, flattenSplit } from "./diffrows";

const L = (origin: string, content = origin): DiffLine => ({ origin, old_lineno: null, new_lineno: null, content });
const hunks: DiffHunk[] = [
  { header: "@@ -1 +1 @@", lines: [L(" ", "ctx"), L("-", "a"), L("-", "b"), L("+", "c")] },
  { header: "@@ -9 +9 @@", lines: [L("+", "d")] },
];

describe("buildSplitRows", () => {
  it("spans context and pairs deletions with additions", () => {
    const rows = buildSplitRows(hunks);
    expect(rows[0]).toEqual([
      { left: { l: hunks[0].lines[0], li: 0 }, right: { l: hunks[0].lines[0], li: 0 }, ctx: true },
      { left: { l: hunks[0].lines[1], li: 1 }, right: { l: hunks[0].lines[3], li: 3 } },
      { left: { l: hunks[0].lines[2], li: 2 }, right: undefined },
    ]);
    expect(rows[1]).toEqual([{ left: undefined, right: { l: hunks[1].lines[0], li: 0 } }]);
  });
});

describe("flattenUnified", () => {
  it("emits a head per hunk followed by its lines", () => {
    expect(flattenUnified(hunks)).toEqual([
      { kind: "head", hi: 0 },
      { kind: "line", hi: 0, li: 0, l: hunks[0].lines[0] },
      { kind: "line", hi: 0, li: 1, l: hunks[0].lines[1] },
      { kind: "line", hi: 0, li: 2, l: hunks[0].lines[2] },
      { kind: "line", hi: 0, li: 3, l: hunks[0].lines[3] },
      { kind: "head", hi: 1 },
      { kind: "line", hi: 1, li: 0, l: hunks[1].lines[0] },
    ]);
  });
});

describe("flattenSplit", () => {
  it("emits a head per hunk followed by its paired rows", () => {
    const rows = buildSplitRows(hunks);
    const flat = flattenSplit(hunks, rows);
    expect(flat.map((r) => (r.kind === "head" ? `H${r.hi}` : `${r.hi}:${r.ri}`))).toEqual([
      "H0", "0:0", "0:1", "0:2", "H1", "1:0",
    ]);
    expect(flat[1]).toEqual({ kind: "row", hi: 0, ri: 0, row: rows[0][0] });
  });
});
