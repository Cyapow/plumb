import { describe, it, expect } from "vitest";
import { highlightLine, langFromPath } from "./highlight";

describe("langFromPath", () => {
  it("maps extensions", () => {
    expect(langFromPath("src/a.ts")).toBe("typescript");
    expect(langFromPath("Makefile")).toBe("makefile");
    expect(langFromPath("x.unknown")).toBeUndefined();
  });
});

describe("highlightLine", () => {
  it("highlights a short line", () => {
    expect(highlightLine("const a = 1;", "typescript")).toContain("hljs-keyword");
  });

  it("escapes without highlighting when there is no language", () => {
    expect(highlightLine("<b>", undefined)).toBe("&lt;b&gt;");
  });

  it("escapes without highlighting lines over 2000 chars", () => {
    const line = "const a = '<' + " + "x".repeat(2000) + ";";
    const out = highlightLine(line, "typescript");
    expect(out).not.toContain("hljs-");
    expect(out).toContain("&lt;");
  });
});
