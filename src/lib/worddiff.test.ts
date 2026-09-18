import { describe, it, expect } from "vitest";
import { wordDiff } from "./worddiff";

describe("wordDiff", () => {
  it("marks only the changed words in a normal pair", () => {
    const [del, add] = wordDiff("const a = 1;", "const b = 1;");
    expect(del).toEqual([
      { text: "const ", changed: false },
      { text: "a", changed: true },
      { text: " = 1;", changed: false },
    ]);
    expect(add).toEqual([
      { text: "const ", changed: false },
      { text: "b", changed: true },
      { text: " = 1;", changed: false },
    ]);
  });

  it("returns whole-line segments when one side is empty", () => {
    expect(wordDiff("", "x")).toEqual([[], [{ text: "x", changed: true }]]);
    expect(wordDiff("x", "")).toEqual([[{ text: "x", changed: true }], []]);
  });

  it("skips the LCS table above the cell budget", () => {
    // Below the guard: the shared prefix is detected.
    const [d1] = wordDiff("x y", "x z");
    expect(d1).toEqual([
      { text: "x ", changed: false },
      { text: "y", changed: true },
    ]);
    // 1201 tokens per side (600 words + 600 spaces + 1) → ~1.4M cells, over
    // the 250k guard: the identical shared prefix is NOT detected and the
    // whole line comes back as one changed segment.
    const shared = Array.from({ length: 600 }, (_, i) => `t${i}`).join(" ");
    const [d2, a2] = wordDiff(shared + " y", shared + " z");
    expect(d2).toEqual([{ text: shared + " y", changed: true }]);
    expect(a2).toEqual([{ text: shared + " z", changed: true }]);
  });

  it("skips tokenising lines over 2000 chars", () => {
    // 2002 chars but only 3 tokens per side (~9 LCS cells) — well under
    // MAX_CELLS, so only the character-length pre-check can catch this.
    const shared = "a".repeat(2000) + " ";
    const [d3, a3] = wordDiff(shared + "y", shared + "z");
    expect(d3).toEqual([{ text: shared + "y", changed: true }]);
    expect(a3).toEqual([{ text: shared + "z", changed: true }]);
  });
});
