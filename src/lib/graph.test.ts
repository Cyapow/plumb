import { describe, expect, it } from "vitest";
import { layoutGraph, ROW_H } from "./graph";
import type { CommitRow } from "./git";

const row = (id: string, parents: string[]): CommitRow =>
  ({ id, parents, refs: [], is_merge: parents.length > 1 }) as unknown as CommitRow;

describe("layoutGraph rowSeg", () => {
  it("indexes each row's segments so a window can slice them", () => {
    // d → merge of c and b; c → a; b → a
    const commits = [row("d", ["c", "b"]), row("c", ["a"]), row("b", ["a"]), row("a", [])];
    const g = layoutGraph(commits);
    expect(g.rowSeg).toHaveLength(commits.length + 1);
    expect(g.rowSeg[0]).toBe(0);
    expect(g.rowSeg[commits.length]).toBe(g.segments.length);
    for (let r = 0; r < commits.length; r++) {
      for (const s of g.segments.slice(g.rowSeg[r], g.rowSeg[r + 1])) {
        expect(s.y1).toBeGreaterThanOrEqual(r * ROW_H);
        expect(s.y2).toBeLessThanOrEqual((r + 1) * ROW_H);
      }
    }
  });
});
