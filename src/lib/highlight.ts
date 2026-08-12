// Per-line syntax highlighting for the diff viewer, using highlight.js's
// "common" bundle (~35 languages). Highlighting is per-line, so multi-line
// constructs (block comments, template strings) aren't tracked across lines —
// the pragmatic trade-off every diff viewer makes.
import hljs from "highlight.js/lib/common";

const MAP: Record<string, string> = {
  ts: "typescript", tsx: "typescript", mts: "typescript", cts: "typescript",
  js: "javascript", jsx: "javascript", mjs: "javascript", cjs: "javascript",
  vue: "xml", svelte: "xml", html: "xml", xml: "xml", svg: "xml",
  json: "json", jsonc: "json",
  rs: "rust", py: "python", rb: "ruby", go: "go", java: "java", kt: "kotlin",
  c: "c", h: "c", cpp: "cpp", cc: "cpp", cxx: "cpp", hpp: "cpp", cs: "csharp",
  php: "php", swift: "swift", lua: "lua", r: "r", pl: "perl", m: "objectivec",
  css: "css", scss: "scss", sass: "scss", less: "less",
  md: "markdown", markdown: "markdown",
  sh: "bash", bash: "bash", zsh: "bash", fish: "bash",
  yml: "yaml", yaml: "yaml", toml: "ini", ini: "ini", cfg: "ini", conf: "ini",
  sql: "sql", vb: "vbnet",
};

export function langFromPath(path: string | null | undefined): string | undefined {
  if (!path) return undefined;
  const base = path.split("/").pop() ?? "";
  if (/^makefile$/i.test(base)) return "makefile";
  const ext = base.includes(".") ? base.split(".").pop()!.toLowerCase() : "";
  return MAP[ext];
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

export function highlightLine(content: string, lang: string | undefined): string {
  if (!content) return "";
  if (lang && hljs.getLanguage(lang)) {
    try {
      return hljs.highlight(content, { language: lang, ignoreIllegals: true }).value;
    } catch {
      return escapeHtml(content);
    }
  }
  return escapeHtml(content);
}
