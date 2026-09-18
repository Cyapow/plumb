# Large diffs + scrollable tab bar — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Large changed files (lockfiles, minified bundles) no longer crash or hang the diff viewer, and the open-repo tab bar scrolls horizontally instead of wrapping.

**Architecture:** Three layers for diffs — (1) cap the O(n·m) word-diff and per-line highlighting so a single long line can't blow up, (2) a Rust-side line-count gate that withholds >20k-line diffs until the user clicks "Show anyway", (3) row windowing in `DiffBody` above 1,500 lines so only visible rows are rendered/highlighted. Row shaping moves to a pure `src/lib/diffrows.ts`. The tab bar becomes an `overflow-x` strip with `‹ ›` buttons that appear on overflow.

**Tech Stack:** Vue 3 `<script setup>` + TypeScript, Tauri 2 (Rust, git2), vitest (new devDependency), cargo test.

**Spec:** `docs/superpowers/specs/2026-09-18-large-diffs-tab-scroll-design.md`

**Branch:** `feat/large-diffs-tab-scroll` (already created, spec committed).

**One deliberate deviation from spec §4:** rather than keeping two templates (nested `hunks → lines` below 1,500 lines, flat windowed list above), `DiffBody` always renders from the flat row list; below the threshold the "window" is simply every row and the spacers are 0px. Same visual result, one template to keep correct. The *behaviour* (no windowing below 1,500 lines) matches the spec.

---

## File map

| File | Change |
|---|---|
| `package.json`, `vite.config.ts` | add vitest + `test` script |
| `src/lib/worddiff.ts` | size guard |
| `src/lib/worddiff.test.ts` | new |
| `src/lib/highlight.ts` | long-line guard |
| `src/lib/highlight.test.ts` | new |
| `src-tauri/src/git.rs` | `FileDiff.truncated/total_lines`, `force` param, gate, test |
| `src-tauri/src/serve.rs` | pass `force` through |
| `src/lib/git.ts` | `FileDiff` type, `force` args |
| `src/lib/ui.ts` | `fullscreen.load(file, force?)` |
| `src/lib/diffrows.ts` | new: `buildSplitRows`, `flattenUnified`, `flattenSplit` |
| `src/lib/diffrows.test.ts` | new |
| `src/components/DiffBody.vue` | placeholder, flat rows, windowing |
| `src/components/DiffView.vue` | `force` + reload |
| `src/components/DiffFullscreen.vue` | `force` + reload |
| `src/components/ChangesView.vue`, `CommitDetail.vue`, `CompareDialog.vue` | forward `force` in `load` |
| `src/App.vue` | tab strip markup, CSS, scroll state |

---

### Task 1: vitest setup

**Files:**
- Modify: `package.json`
- Modify: `vite.config.ts`
- Create: `src/lib/smoke.test.ts` (deleted again in Task 2)

- [ ] **Step 1: Install vitest**

Run: `cd /Users/mat/Code/Cyapow/plumb && npm i -D vitest@^3`
Expected: `package.json` devDependencies gains `"vitest": "^3.x"`.

- [ ] **Step 2: Add the test script**

In `package.json` `scripts`, add after `"preview"`:

```json
    "test": "vitest run",
```

- [ ] **Step 3: Point vite.config at vitest's defineConfig**

Replace the first line of `vite.config.ts`:

```ts
import { defineConfig } from "vitest/config";
```

and add a `test` block inside the returned object, after `plugins: [vue()],`:

```ts
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
```

- [ ] **Step 4: Smoke test**

Create `src/lib/smoke.test.ts`:

```ts
import { describe, it, expect } from "vitest";

describe("vitest", () => {
  it("runs", () => {
    expect(1 + 1).toBe(2);
  });
});
```

Run: `npm test`
Expected: `1 passed`.

- [ ] **Step 5: Type-check still passes**

Run: `npx vue-tsc --noEmit`
Expected: no output (exit 0).

- [ ] **Step 6: Commit**

```bash
git add package.json package-lock.json vite.config.ts src/lib/smoke.test.ts
git commit -m "chore: add vitest"
```

---

### Task 2: `wordDiff` size guard

**Files:**
- Modify: `src/lib/worddiff.ts`
- Create: `src/lib/worddiff.test.ts`
- Delete: `src/lib/smoke.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `src/lib/worddiff.test.ts`:

```ts
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
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `rm src/lib/smoke.test.ts && npx vitest run src/lib/worddiff.test.ts`
Expected: first two pass; "skips the LCS table above the cell budget" FAILS — the LCS currently finds the shared prefix and marks only `y`/`z`.

- [ ] **Step 3: Implement the guard**

In `src/lib/worddiff.ts`, replace the top of `wordDiff` (up to and including the `const m = bt.length;` line) with:

```ts
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
```

Leave the rest of the function as is.

- [ ] **Step 4: Run to verify it passes**

Run: `npx vitest run src/lib/worddiff.test.ts`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add src/lib/worddiff.ts src/lib/worddiff.test.ts
git rm -q src/lib/smoke.test.ts
git commit -m "fix(diff): cap word-diff LCS table so minified lines can't OOM"
```

---

### Task 3: `highlightLine` long-line guard

**Files:**
- Modify: `src/lib/highlight.ts`
- Create: `src/lib/highlight.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `src/lib/highlight.test.ts`:

```ts
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
```

- [ ] **Step 2: Run to verify it fails**

Run: `npx vitest run src/lib/highlight.test.ts`
Expected: "over 2000 chars" FAILS (`hljs-keyword` present).

- [ ] **Step 3: Implement the guard**

In `src/lib/highlight.ts`, replace `export function highlightLine` with:

```ts
/** Lines longer than this are almost always minified/generated; highlighting
 *  them is slow and adds nothing, so they're escaped and left plain. */
const MAX_HL_CHARS = 2000;

export function highlightLine(content: string, lang: string | undefined): string {
  if (!content) return "";
  if (lang && content.length <= MAX_HL_CHARS && hljs.getLanguage(lang)) {
    try {
      return hljs.highlight(content, { language: lang, ignoreIllegals: true }).value;
    } catch {
      return escapeHtml(content);
    }
  }
  return escapeHtml(content);
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `npx vitest run src/lib/highlight.test.ts`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add src/lib/highlight.ts src/lib/highlight.test.ts
git commit -m "fix(diff): skip syntax highlighting on lines over 2000 chars"
```

---

### Task 4: Rust size gate

**Files:**
- Modify: `src-tauri/src/git.rs` (`FileDiff` ~line 116, `file_diff` ~456, `collect_file_diff` ~481, `compare_file_diff` ~1434, `commit_file_diff` ~1499, tests module ~2967)
- Modify: `src-tauri/src/serve.rs:401-418`

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/git.rs`, inside `mod tests`, add after `init_names_branch_and_reports_unborn`:

```rust
    #[test]
    fn file_diff_gates_huge_diffs_unless_forced() {
        let d = tmp();
        init_repo(p(&d), None).unwrap();
        let big: String = (0..MAX_DIFF_LINES + 1).map(|i| format!("line {i}\n")).collect();
        std::fs::write(d.path().join("big.txt"), big).unwrap();

        let gated = file_diff(p(&d), "big.txt".into(), false, None).unwrap();
        assert!(gated.truncated);
        assert_eq!(gated.total_lines, MAX_DIFF_LINES + 1);
        assert!(gated.hunks.is_empty());

        let forced = file_diff(p(&d), "big.txt".into(), false, Some(true)).unwrap();
        assert!(!forced.truncated);
        assert_eq!(forced.total_lines, MAX_DIFF_LINES + 1);
        let lines: usize = forced.hunks.iter().map(|h| h.lines.len()).sum();
        assert_eq!(lines, MAX_DIFF_LINES + 1);

        std::fs::write(d.path().join("small.txt"), "one\ntwo\n").unwrap();
        let small = file_diff(p(&d), "small.txt".into(), false, None).unwrap();
        assert!(!small.truncated);
        assert_eq!(small.total_lines, 2);
        assert_eq!(small.hunks[0].lines.len(), 2);
    }
```

- [ ] **Step 2: Run to verify it fails to compile**

Run: `cd src-tauri && cargo test file_diff_gates 2>&1 | tail -20`
Expected: compile errors — `MAX_DIFF_LINES` not found, `file_diff` takes 3 arguments, no field `truncated`.

- [ ] **Step 3: Extend `FileDiff`**

Replace the `FileDiff` struct:

```rust
/// A single file's diff, either staged (HEAD↔index) or unstaged (index↔workdir).
#[derive(Serialize)]
pub struct FileDiff {
    pub path: String,
    pub staged: bool,
    pub binary: bool,
    pub hunks: Vec<DiffHunk>,
    /// True when the diff exceeded `MAX_DIFF_LINES` and `force` wasn't set;
    /// `hunks` is empty and the UI offers "Show anyway".
    pub truncated: bool,
    /// Total diff lines (context + added + removed), populated even when truncated.
    pub total_lines: usize,
}
```

- [ ] **Step 4: Gate in `collect_file_diff`**

Replace `collect_file_diff` in full:

```rust
/// Diffs with more lines than this are withheld until the caller passes
/// `force` — serialising and rendering them is what used to freeze the UI.
pub const MAX_DIFF_LINES: usize = 20_000;

/// Turn a libgit2 diff into a single file's hunks/lines. With `force == false`
/// a diff over `MAX_DIFF_LINES` comes back with `truncated: true` and no hunks.
fn collect_file_diff(diff: &git2::Diff, file: &str, staged: bool, force: bool) -> Result<FileDiff> {
    let mut result = FileDiff {
        path: file.to_string(),
        staged,
        binary: false,
        hunks: Vec::new(),
        truncated: false,
        total_lines: 0,
    };

    for i in 0..diff.deltas().len() {
        let delta = diff.get_delta(i);
        let matches = delta
            .and_then(|d| d.new_file().path().or_else(|| d.old_file().path()))
            .map(|p| p.to_string_lossy() == file)
            .unwrap_or(false);
        if !matches {
            continue;
        }

        match Patch::from_diff(diff, i)? {
            None => result.binary = true,
            Some(patch) => {
                let hunk_count = patch.num_hunks();
                result.total_lines = (0..hunk_count)
                    .map(|h| patch.num_lines_in_hunk(h).unwrap_or(0))
                    .sum();
                if result.total_lines > MAX_DIFF_LINES && !force {
                    result.truncated = true;
                    break;
                }
                for h in 0..hunk_count {
                    let (hunk, _) = patch.hunk(h)?;
                    let header = String::from_utf8_lossy(hunk.header()).trim_end().to_string();
                    let mut lines = Vec::new();
                    for l in 0..patch.num_lines_in_hunk(h)? {
                        let dl = patch.line_in_hunk(h, l)?;
                        lines.push(DiffLine {
                            origin: dl.origin().to_string(),
                            old_lineno: dl.old_lineno(),
                            new_lineno: dl.new_lineno(),
                            content: String::from_utf8_lossy(dl.content())
                                .trim_end_matches('\n')
                                .to_string(),
                        });
                    }
                    result.hunks.push(DiffHunk { header, lines });
                }
            }
        }
        break;
    }
    Ok(result)
}
```

- [ ] **Step 5: Thread `force` through the three commands**

`file_diff` signature and last line:

```rust
pub fn file_diff(path: String, file: String, staged: bool, force: Option<bool>) -> Result<FileDiff> {
```
```rust
    collect_file_diff(&diff, &file, staged, force.unwrap_or(false))
```

`compare_file_diff`:

```rust
pub fn compare_file_diff(path: String, base: String, compare: String, file: String, force: Option<bool>) -> Result<FileDiff> {
```
```rust
    collect_file_diff(&diff, &file, false, force.unwrap_or(false))
```

`commit_file_diff`:

```rust
pub fn commit_file_diff(path: String, id: String, file: String, force: Option<bool>) -> Result<FileDiff> {
```
```rust
    collect_file_diff(&diff, &file, false, force.unwrap_or(false))
```

- [ ] **Step 6: Served mode passes `force`**

In `src-tauri/src/serve.rs` the three match arms become:

```rust
        "commit_file_diff" => ok(git::commit_file_diff(s("path"), s("id"), s("file"), args["force"].as_bool())),
        "file_diff" => ok(git::file_diff(s("path"), s("file"), b("staged"), args["force"].as_bool())),
```
```rust
        "compare_file_diff" => ok(git::compare_file_diff(s("path"), s("base"), s("compare"), s("file"), args["force"].as_bool())),
```

- [ ] **Step 7: Run the test**

Run: `cd src-tauri && cargo test file_diff_gates 2>&1 | tail -5`
Expected: `test git::tests::file_diff_gates_huge_diffs_unless_forced ... ok`.

Run: `cargo test 2>&1 | tail -3`
Expected: all tests pass, no warnings about unused `force`.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/git.rs src-tauri/src/serve.rs
git commit -m "feat(diff): withhold diffs over 20k lines unless forced"
```

---

### Task 5: TypeScript wrappers

**Files:**
- Modify: `src/lib/git.ts:137-142, 335-337, 420-422, 432-434`

- [ ] **Step 1: Update the type**

```ts
export interface FileDiff {
  path: string;
  staged: boolean;
  binary: boolean;
  hunks: DiffHunk[];
  /** Diff exceeded the backend line cap and `force` wasn't set; hunks is empty. */
  truncated: boolean;
  total_lines: number;
}
```

- [ ] **Step 2: Add `force` to the three wrappers**

```ts
export function fileDiff(path: string, file: string, staged: boolean, force = false): Promise<FileDiff> {
  return invoke("file_diff", { path, file, staged, force });
}
```
```ts
export function commitFileDiff(path: string, id: string, file: string, force = false): Promise<FileDiff> {
  return invoke("commit_file_diff", { path, id, file, force });
}
```
```ts
export function compareFileDiff(path: string, base: string, compare: string, file: string, force = false): Promise<FileDiff> {
  return invoke("compare_file_diff", { path, base, compare, file, force });
}
```

- [ ] **Step 3: Type-check**

Run: `npx vue-tsc --noEmit`
Expected: exit 0.

- [ ] **Step 4: Commit**

```bash
git add src/lib/git.ts
git commit -m "feat(diff): expose truncated/total_lines and force on diff wrappers"
```

---

### Task 6: "Too big" placeholder and reload

**Files:**
- Modify: `src/components/DiffBody.vue` (props, emits, template)
- Modify: `src/components/DiffView.vue`
- Modify: `src/lib/ui.ts:160,169,177`
- Modify: `src/components/DiffFullscreen.vue`
- Modify: `src/components/ChangesView.vue:359-362`, `src/components/CommitDetail.vue:71`, `src/components/CompareDialog.vue:45`

- [ ] **Step 1: DiffBody props + emit + placeholder**

In `DiffBody.vue` `defineProps`, add after `filePath?`:

```ts
  truncated?: boolean; // backend withheld the hunks (over its line cap)
  totalLines?: number;
```

In `defineEmits`, add:

```ts
  (e: "showAnyway"): void;
```

In the template, after the `binary` branch and before the `hunks.length === 0` branch:

```html
    <div v-else-if="truncated" class="empty">
      <div>Large diff — {{ (totalLines ?? 0).toLocaleString() }} lines. Rendering may be slow.</div>
      <button class="hunk-btn show-anyway" @click="emit('showAnyway')">Show anyway</button>
    </div>
```

In `<style scoped>`, after `.empty`:

```css
.empty .show-anyway { margin-top: var(--space-3); }
```

- [ ] **Step 2: DiffView force + reload**

Replace the `<script setup>` body from `const diff = ref…` to the end of the `watch` with:

```ts
const diff = ref<FileDiff | null>(null);
const loading = ref(false);
// "Show anyway" opt-in for diffs over the backend line cap. Reset when the
// file changes, but not on refresh/reload so a stage/unstage keeps it open.
const force = ref(false);
watch(() => [props.file, props.staged, props.repoPath] as const, () => (force.value = false));

async function load() {
  if (!props.file) {
    diff.value = null;
    return;
  }
  loading.value = true;
  try {
    diff.value = await fileDiff(props.repoPath, props.file, props.staged, force.value);
  } catch {
    diff.value = null;
  } finally {
    loading.value = false;
  }
}
watch(() => [props.file, props.staged, props.repoPath, props.refresh, diffReloadKey.value] as const, load, {
  immediate: true,
});
function showAnyway() {
  force.value = true;
  void load();
}
```

In the template, add to `<DiffBody>`:

```html
      :truncated="diff?.truncated"
      :total-lines="diff?.total_lines"
      @show-anyway="showAnyway"
```

- [ ] **Step 3: `fullscreen.load` signature**

In `src/lib/ui.ts`, change both occurrences of the `load` type:

```ts
  load: ((file: string, force?: boolean) => Promise<FileDiff>) | null;
```
```ts
  load: (file: string, force?: boolean) => Promise<FileDiff>;
```

- [ ] **Step 4: DiffFullscreen force + reload**

Replace `const diff = ref…` through the `watch(...)` line with:

```ts
const diff = ref<FileDiff | null>(null);
const loading = ref(false);
const listW = ref(320);
// "Show anyway" opt-in for diffs over the backend line cap; reset per file.
const force = ref(false);
watch(() => fullscreen.activeFile, () => (force.value = false));

async function loadActive() {
  if (!fullscreen.load || !fullscreen.activeFile) {
    diff.value = null;
    return;
  }
  loading.value = true;
  try {
    diff.value = await fullscreen.load(fullscreen.activeFile, force.value);
  } catch {
    diff.value = null;
  } finally {
    loading.value = false;
  }
}
function showAnyway() {
  force.value = true;
  void loadActive();
}

watch(() => [fullscreen.open, fullscreen.activeFile, diffReloadKey.value], loadActive, { immediate: true });
```

Add to the `<DiffBody>` in the template:

```html
        :truncated="diff?.truncated"
        :total-lines="diff?.total_lines"
        @show-anyway="showAnyway"
```

- [ ] **Step 5: Forward `force` from the three `openFullscreen` callers**

`ChangesView.vue`:

```ts
    load: (file, force) => {
      const entry = files.value.find((f) => f.path === file);
      return fileDiff(props.repoPath, file, entry ? diffStaged(entry) : false, force);
    },
```

`CommitDetail.vue`:

```ts
    load: (f, force) => commitFileDiff(props.repoPath, id, f, force),
```

`CompareDialog.vue`:

```ts
      load: (file, force) => compareFileDiff(props.repoPath, b, c, file, force),
```

- [ ] **Step 6: Type-check and eyeball**

Run: `npx vue-tsc --noEmit`
Expected: exit 0.

Run: `npm run tauri dev`, open a repo, make `package-lock.json` differ by >20k lines (e.g. `printf 'x\n%.0s' {1..20001} >> package-lock.json` in some scratch repo — **not this one**), select it in Changes.
Expected: "Large diff — 20,001 lines…" with a Show anyway button; clicking it renders the diff (slowly — Task 8 fixes that). Selecting another file then back → placeholder again.

- [ ] **Step 7: Commit**

```bash
git add src/components/DiffBody.vue src/components/DiffView.vue src/components/DiffFullscreen.vue src/components/ChangesView.vue src/components/CommitDetail.vue src/components/CompareDialog.vue src/lib/ui.ts
git commit -m "feat(diff): 'Show anyway' placeholder for diffs over the line cap"
```

---

### Task 7: Pure row model `src/lib/diffrows.ts`

**Files:**
- Create: `src/lib/diffrows.ts`
- Create: `src/lib/diffrows.test.ts`
- Modify: `src/components/DiffBody.vue` (use `buildSplitRows`)

- [ ] **Step 1: Write the failing tests**

Create `src/lib/diffrows.test.ts`:

```ts
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
```

- [ ] **Step 2: Run to verify it fails**

Run: `npx vitest run src/lib/diffrows.test.ts`
Expected: FAIL — cannot resolve `./diffrows`.

- [ ] **Step 3: Implement**

Create `src/lib/diffrows.ts`:

```ts
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
```

- [ ] **Step 4: Run to verify it passes**

Run: `npx vitest run src/lib/diffrows.test.ts`
Expected: 3 passed.

- [ ] **Step 5: Use `buildSplitRows` in DiffBody**

In `DiffBody.vue`, replace the `interface Cell`, `interface SplitRow` and the whole `const splitRows = computed<SplitRow[][]>(() => …)` block with:

```ts
const splitRows = computed(() => buildSplitRows(props.hunks));
```

and add to the imports:

```ts
import { buildSplitRows } from "../lib/diffrows";
```

Remove the now-unused `DiffLine` type import if vue-tsc flags it (keep `DiffHunk`).

- [ ] **Step 6: Type-check + tests**

Run: `npx vue-tsc --noEmit && npm test`
Expected: exit 0; all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/lib/diffrows.ts src/lib/diffrows.test.ts src/components/DiffBody.vue
git commit -m "refactor(diff): move row shaping into a pure diffrows module"
```

---

### Task 8: Windowed rendering in DiffBody

**Files:**
- Modify: `src/components/DiffBody.vue` (script + template + style)

This is the largest task. The template switches from nested `hunks → lines` loops to one flat `visible*` list per view with spacer divs. Below 1,500 lines `visible*` is every row and the spacers are 0px.

- [ ] **Step 1: Script — windowing state**

Replace the whole `<script setup>` of `DiffBody.vue` with:

```ts
<script setup lang="ts">
// Presentational diff renderer — blue/red, line-numbered. Optional per-hunk
// action button, and optional line selection for line-level staging.
//
// Above VIRTUAL_MIN_LINES only the rows near the viewport are rendered (row
// windowing): a lockfile diff of 30k lines would otherwise mean 30k DOM rows
// and 30k highlight.js calls up front.
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import type { DiffHunk } from "../lib/git";
import { highlightLine, langFromPath } from "../lib/highlight";
import { wordDiff, type Seg } from "../lib/worddiff";
import { buildSplitRows, flattenSplit, flattenUnified } from "../lib/diffrows";
import { prefs } from "../lib/ui";

const props = defineProps<{
  hunks: DiffHunk[];
  binary?: boolean;
  loading?: boolean;
  emptyText?: string;
  actionLabel?: string; // e.g. "Stage hunk" / "Unstage hunk"
  selectable?: boolean; // enable per-line selection
  filePath?: string | null; // for syntax highlighting
  truncated?: boolean; // backend withheld the hunks (over its line cap)
  totalLines?: number;
}>();

const emit = defineEmits<{
  (e: "hunkAction", index: number): void;
  (e: "lineAction", hunkIndex: number, lines: number[]): void;
  (e: "showAnyway"): void;
}>();

const lang = computed(() => langFromPath(props.filePath));
const hl = (content: string) => highlightLine(content, lang.value);

// Word-level diff for 1:1 changed line pairs (a single "-" followed by "+").
const wordSegs = computed(() => {
  const m = new Map<string, Seg[]>();
  props.hunks.forEach((h, hi) => {
    const lines = h.lines;
    for (let li = 0; li < lines.length; li++) {
      if (
        lines[li].origin === "-" &&
        li + 1 < lines.length &&
        lines[li + 1].origin === "+" &&
        (li + 2 >= lines.length || lines[li + 2].origin !== "+") &&
        (li === 0 || lines[li - 1].origin !== "-")
      ) {
        const [del, add] = wordDiff(lines[li].content, lines[li + 1].content);
        m.set(`${hi}:${li}`, del);
        m.set(`${hi}:${li + 1}`, add);
        li++;
      }
    }
  });
  return m;
});
const segsFor = (hi: number, li: number) => wordSegs.value.get(`${hi}:${li}`);

const cls = (o: string) => (o === "+" ? "add" : o === "-" ? "del" : "ctx");
const verb = () => (props.actionLabel?.startsWith("Unstage") ? "Unstage" : "Stage");

const split = computed(() => prefs.split);

// ── Rows ──
const splitRows = computed(() => buildSplitRows(props.hunks));
const unifiedRows = computed(() => flattenUnified(props.hunks));
const splitItems = computed(() => flattenSplit(props.hunks, splitRows.value));
const rowCount = computed(() => (split.value ? splitItems.value : unifiedRows.value).length);

// ── Windowing ──
const VIRTUAL_MIN_LINES = 1500;
const OVERSCAN = 20;
const lineCount = computed(() => props.hunks.reduce((n, h) => n + h.lines.length, 0));
const virtual = computed(() => lineCount.value > VIRTUAL_MIN_LINES);

const rootEl = ref<HTMLElement | null>(null); // unified scroller
const leftPane = ref<HTMLElement | null>(null); // split scrollers
const rightPane = ref<HTMLElement | null>(null);
const scroller = () => (split.value ? leftPane.value : rootEl.value);

// Row height is line-height × font-size (fractional), so measure it rather
// than hardcode; 20.4 is 12px × 1.7 from tokens.css as a first guess.
const rowH = ref(20.4);
const viewStart = ref(0);
const viewEnd = ref(0);

function updateWindow() {
  if (!virtual.value) return;
  const el = scroller();
  if (!el) return;
  const first = Math.floor(el.scrollTop / rowH.value);
  const count = Math.ceil(el.clientHeight / rowH.value);
  viewStart.value = Math.max(0, first - OVERSCAN);
  viewEnd.value = Math.min(rowCount.value, first + count + OVERSCAN);
}
function measureRow() {
  const r = scroller()?.querySelector<HTMLElement>(".line, .pline");
  if (!r) return;
  const h = r.getBoundingClientRect().height;
  if (h > 0 && Math.abs(h - rowH.value) > 0.01) {
    rowH.value = h;
    updateWindow();
  }
}
let raf = 0;
function onScroll() {
  if (raf) return;
  raf = requestAnimationFrame(() => {
    raf = 0;
    updateWindow();
  });
}

const visibleUnified = computed(() =>
  virtual.value ? unifiedRows.value.slice(viewStart.value, viewEnd.value) : unifiedRows.value,
);
const visibleSplit = computed(() =>
  virtual.value ? splitItems.value.slice(viewStart.value, viewEnd.value) : splitItems.value,
);
const padTop = computed(() => (virtual.value ? viewStart.value * rowH.value : 0));
const padBottom = computed(() => (virtual.value ? Math.max(0, rowCount.value - viewEnd.value) * rowH.value : 0));

// New content or a view switch: render an initial slice, then measure a real
// row and size the window from it.
watch(
  [() => props.hunks, split],
  () => {
    viewStart.value = 0;
    viewEnd.value = Math.min(rowCount.value, OVERSCAN * 2 + 60);
    nextTick(() => {
      measureRow();
      updateWindow();
    });
  },
  { immediate: true, flush: "post" },
);

let ro: ResizeObserver | null = null;
onMounted(() => {
  ro = new ResizeObserver(() => {
    measureRow();
    updateWindow();
  });
  if (rootEl.value) ro.observe(rootEl.value);
});
onUnmounted(() => {
  ro?.disconnect();
  if (raf) cancelAnimationFrame(raf);
});

// Side-by-side panes scroll horizontally on their own; keep their vertical
// scroll in lock-step so the two columns always show the same rows.
let syncing = false;
function onPaneScroll(from: "l" | "r") {
  onScroll();
  if (syncing) return;
  const src = from === "l" ? leftPane.value : rightPane.value;
  const dst = from === "l" ? rightPane.value : leftPane.value;
  if (!src || !dst || dst.scrollTop === src.scrollTop) return;
  syncing = true;
  dst.scrollTop = src.scrollTop;
  requestAnimationFrame(() => (syncing = false));
}

// Selected line indices, keyed "hunk:line".
const picked = ref<Set<string>>(new Set());
watch(
  () => props.hunks,
  () => picked.value.clear(),
);

function toggleLine(hi: number, li: number, origin: string) {
  if (!props.selectable || (origin !== "+" && origin !== "-")) return;
  const k = `${hi}:${li}`;
  const next = new Set(picked.value);
  next.has(k) ? next.delete(k) : next.add(k);
  picked.value = next;
}
const isPicked = (hi: number, li: number) => picked.value.has(`${hi}:${li}`);
function pickedInHunk(hi: number): number[] {
  const out: number[] = [];
  props.hunks[hi]?.lines.forEach((_, li) => {
    if (picked.value.has(`${hi}:${li}`)) out.push(li);
  });
  return out;
}
</script>
```

- [ ] **Step 2: Template — flat rows + spacers**

Replace the whole `<template>` with:

```html
<template>
  <div class="diff" ref="rootEl" @scroll="!split && onScroll()">
    <div v-if="loading" class="empty">Reading diff…</div>
    <div v-else-if="binary" class="empty">Binary file — no textual diff.</div>
    <div v-else-if="truncated" class="empty">
      <div>Large diff — {{ (totalLines ?? 0).toLocaleString() }} lines. Rendering may be slow.</div>
      <button class="hunk-btn show-anyway" @click="emit('showAnyway')">Show anyway</button>
    </div>
    <div v-else-if="hunks.length === 0" class="empty">{{ emptyText ?? "No changes to show." }}</div>
    <!-- Unified -->
    <div v-else-if="!split" class="hunks hunks-unified mono" :class="{ virtual }">
      <div class="spacer" :style="{ height: padTop + 'px' }"></div>
      <template v-for="r in visibleUnified" :key="r.kind === 'head' ? 'h' + r.hi : r.hi + '-' + r.li">
        <div v-if="r.kind === 'head'" class="hunk-head">
          <span class="hh-text">{{ hunks[r.hi].header }}</span>
          <span class="hunk-actions">
            <button
              v-if="selectable && pickedInHunk(r.hi).length"
              class="hunk-btn accent"
              @click="emit('lineAction', r.hi, pickedInHunk(r.hi))"
            >{{ verb() }} {{ pickedInHunk(r.hi).length }} line{{ pickedInHunk(r.hi).length === 1 ? "" : "s" }}</button>
            <button v-if="actionLabel" class="hunk-btn" @click="emit('hunkAction', r.hi)">{{ actionLabel }}</button>
          </span>
        </div>
        <div
          v-else
          class="line"
          :class="[cls(r.l.origin), { picked: isPicked(r.hi, r.li), selectable: selectable && (r.l.origin === '+' || r.l.origin === '-') }]"
          @click="toggleLine(r.hi, r.li, r.l.origin)"
        >
          <span class="ln old">{{ r.l.old_lineno ?? "" }}</span>
          <span class="ln new">{{ r.l.new_lineno ?? "" }}</span>
          <span class="sign">{{ r.l.origin === " " ? "" : r.l.origin }}</span>
          <span v-if="segsFor(r.hi, r.li)" class="content"
            ><span v-for="(s, k) in segsFor(r.hi, r.li)" :key="k" :class="{ word: s.changed }">{{ s.text }}</span></span
          >
          <span v-else class="content" v-html="hl(r.l.content)"></span>
        </div>
      </template>
      <div class="spacer" :style="{ height: padBottom + 'px' }"></div>
    </div>

    <!-- Side-by-side: two independent panes. Each scrolls horizontally on its
         own; their vertical scroll is kept in sync so rows stay aligned. -->
    <div v-else class="split-wrap" :class="{ virtual }">
      <div class="pane pane-left" ref="leftPane" @scroll="onPaneScroll('l')">
        <div class="pane-inner mono">
          <div class="spacer" :style="{ height: padTop + 'px' }"></div>
          <template v-for="r in visibleSplit" :key="r.kind === 'head' ? 'h' + r.hi : r.hi + '-' + r.ri">
            <div v-if="r.kind === 'head'" class="hunk-head">
              <span class="hh-text">{{ hunks[r.hi].header }}</span>
              <span class="hunk-actions">
                <button v-if="actionLabel" class="hunk-btn" @click="emit('hunkAction', r.hi)">{{ actionLabel }}</button>
              </span>
            </div>
            <div v-else class="pline" :class="r.row.ctx ? 'ctx' : r.row.left ? 'del' : 'empty'">
              <span class="ln">{{ r.row.left?.l.old_lineno ?? "" }}</span>
              <span v-if="r.row.left && segsFor(r.hi, r.row.left.li)" class="content"
                ><span v-for="(s, k) in segsFor(r.hi, r.row.left.li)" :key="k" :class="{ word: s.changed }">{{ s.text }}</span></span
              >
              <span v-else-if="r.row.left" class="content" v-html="hl(r.row.left.l.content)"></span>
            </div>
          </template>
          <div class="spacer" :style="{ height: padBottom + 'px' }"></div>
        </div>
      </div>
      <div class="pane pane-right" ref="rightPane" @scroll="onPaneScroll('r')">
        <div class="pane-inner mono">
          <div class="spacer" :style="{ height: padTop + 'px' }"></div>
          <template v-for="r in visibleSplit" :key="r.kind === 'head' ? 'h' + r.hi : r.hi + '-' + r.ri">
            <div v-if="r.kind === 'head'" class="hunk-head"><span class="hh-text">{{ hunks[r.hi].header }}</span></div>
            <div v-else class="pline" :class="r.row.ctx ? 'ctx' : r.row.right ? 'add' : 'empty'">
              <span class="ln">{{ r.row.right?.l.new_lineno ?? "" }}</span>
              <span v-if="r.row.right && segsFor(r.hi, r.row.right.li)" class="content"
                ><span v-for="(s, k) in segsFor(r.hi, r.row.right.li)" :key="k" :class="{ word: s.changed }">{{ s.text }}</span></span
              >
              <span v-else-if="r.row.right" class="content" v-html="hl(r.row.right.l.content)"></span>
            </div>
          </template>
          <div class="spacer" :style="{ height: padBottom + 'px' }"></div>
        </div>
      </div>
    </div>
  </div>
</template>
```

Note `$emit` → `emit` throughout (both existed before; `emit` is the one defined in script).

- [ ] **Step 3: Style — uniform row height in virtual mode**

In `<style scoped>`, add after the `.hunk-btn.accent` rule:

```css
/* Windowed mode assumes every row is exactly one line tall so the spacer
   maths is exact: hunk headers lose the button's vertical margin. */
.spacer { flex: none; }
.virtual .hunk-head { height: calc(var(--code-line-h) * 1em); overflow: hidden; }
.virtual .hunk-btn { margin: 0; padding: 0 8px; line-height: 16px; }
```

and keep the `.empty .show-anyway` rule from Task 6.

- [ ] **Step 4: Type-check + tests**

Run: `npx vue-tsc --noEmit && npm test`
Expected: exit 0; all pass. If vue-tsc complains that `r.l` / `r.row` doesn't exist on the union, the `v-if="r.kind === 'head'"` / `v-else` narrowing isn't applying — check that the `v-else` is on the sibling element directly (no whitespace-only text nodes between them break narrowing? they don't, but a comment node between does).

- [ ] **Step 5: Manual QA in the app**

Run: `npm run tauri dev`. In a scratch repo with a `package-lock.json` diff of ~5k lines and one with >20k lines, plus a minified `.js` file with one edited 200 kB line:

- 5k-line diff (unified): renders immediately; scrolling is smooth; scrolling to the bottom shows the last lines with no gap/overlap at hunk boundaries; hunk header buttons visible and one line tall.
- Toggle split: both panes show the same rows, scroll stays synced, no gap at the bottom.
- Click lines in a >1,500-line unstaged diff → "Stage N lines" button appears on that hunk head; clicking stages them.
- >20k-line diff → placeholder → Show anyway → renders windowed.
- Minified file: opens instantly, changed line is plain (unhighlighted) with the whole line marked changed. No freeze.
- Small diff (<1,500 lines): looks exactly as before (hunk header height unchanged, buttons unchanged).
- Resize the window / drag the sidebar splitter: rows fill the new height.

- [ ] **Step 6: Commit**

```bash
git add src/components/DiffBody.vue
git commit -m "perf(diff): window diff rows above 1,500 lines"
```

---

### Task 9: Scrollable tab bar

**Files:**
- Modify: `src/App.vue` (template ~1809-1826, script near `selectTab`/`loadRepo` ~452-495 and `onMounted`/`onUnmounted` ~1423-1456, CSS ~2387-2413)

- [ ] **Step 1: Template**

Replace the tab bar header's tab list (from the `<button v-for="t in tabs"` through the `add-tab` button) with:

```html
      <div class="tabs-scroll" ref="tabsEl" data-tauri-drag-region @scroll="updateTabScroll" @wheel="onTabsWheel">
        <button
          v-for="t in tabs"
          :key="t.path"
          class="repo-tab"
          :class="{ on: showWorkspace && activePath === t.path }"
          :title="t.path"
          :data-path="t.path"
          @click="selectTab(t.path)"
        >
          <span class="tab-name ellipsis">{{ t.name }}</span>
          <span class="tab-x" title="Close" @click.stop="closeTab(t.path)">✕</span>
        </button>
      </div>
      <button v-show="tabsOverflow" class="tab-arrow" :disabled="!canScrollL" title="Scroll tabs left" @click="scrollTabs(-1)">‹</button>
      <button v-show="tabsOverflow" class="tab-arrow" :disabled="!canScrollR" title="Scroll tabs right" @click="scrollTabs(1)">›</button>
      <button class="add-tab" title="Open a repository" @click="goHome">+</button>
```

- [ ] **Step 2: Script — scroll state**

Add directly after the `goHome` function (~line 478):

```ts
// ── Tab strip overflow ──
// Tabs never wrap: the strip scrolls horizontally, with ‹ › buttons that only
// appear once there are more tabs than fit.
const tabsEl = ref<HTMLElement | null>(null);
const tabsOverflow = ref(false);
const canScrollL = ref(false);
const canScrollR = ref(false);
function updateTabScroll() {
  const el = tabsEl.value;
  if (!el) return;
  tabsOverflow.value = el.scrollWidth > el.clientWidth + 1;
  canScrollL.value = el.scrollLeft > 0;
  canScrollR.value = el.scrollLeft + el.clientWidth < el.scrollWidth - 1;
}
function scrollTabs(dir: -1 | 1) {
  tabsEl.value?.scrollBy({ left: dir * 200, behavior: "smooth" });
}
// A plain mouse wheel only produces deltaY; turn it into horizontal scroll.
function onTabsWheel(e: WheelEvent) {
  const el = tabsEl.value;
  if (!el || !tabsOverflow.value || Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;
  e.preventDefault();
  el.scrollLeft += e.deltaY;
}
function revealActiveTab() {
  nextTick(() => {
    const el = tabsEl.value?.querySelector<HTMLElement>(`[data-path="${CSS.escape(activePath.value)}"]`);
    el?.scrollIntoView({ inline: "nearest", block: "nearest" });
  });
}
let tabsRo: ResizeObserver | null = null;
watch(tabs, () => nextTick(updateTabScroll), { deep: true });
watch(activePath, revealActiveTab);
```

In `onMounted`, add after `window.addEventListener("keydown", onHistoryKey);`:

```ts
  tabsRo = new ResizeObserver(updateTabScroll);
  if (tabsEl.value) tabsRo.observe(tabsEl.value);
  updateTabScroll();
```

In `onUnmounted`, add:

```ts
  tabsRo?.disconnect();
```

- [ ] **Step 3: CSS**

Replace the `.repo-tab { padding: 0 12px; gap: var(--space-2); max-width: 220px; }` rule and add the strip/arrow rules, so the block after `.home-tab { padding: 0 14px; }` reads:

```css
.tabs-scroll {
  display: flex; align-items: stretch;
  flex: 0 1 auto; min-width: 0;
  overflow-x: auto; overflow-y: hidden;
  scrollbar-width: none;
}
.tabs-scroll::-webkit-scrollbar { display: none; }
.repo-tab { flex: none; padding: 0 12px; gap: var(--space-2); max-width: 220px; }
.tab-arrow {
  display: flex; align-items: center; justify-content: center;
  flex: none; width: 24px; background: transparent; border: none;
  border-right: 1px solid var(--line); cursor: pointer;
  color: var(--text-dim); font-size: 16px; line-height: 1;
}
.tab-arrow:hover:not(:disabled) { color: var(--text); }
.tab-arrow:disabled { color: var(--text-faint); cursor: default; }
```

- [ ] **Step 4: Type-check**

Run: `npx vue-tsc --noEmit`
Expected: exit 0.

- [ ] **Step 5: Manual QA**

Run: `npm run tauri dev`. Open ~12 repos (Home → recents), then narrow the window:

- Tabs stay on one row; the strip clips; `‹ ›` appear at the right of the strip, before `+`.
- `‹` disabled at the left end, `›` disabled at the right end; clicking scrolls ~200px smoothly.
- Mouse wheel over the tabs scrolls them horizontally; trackpad horizontal swipe also works.
- Click a tab that's partly clipped → it scrolls fully into view. Open a new repo → its tab is scrolled into view.
- Widen the window until everything fits → arrows disappear.
- Dragging the window from empty space in the tab bar (right of the last tab, or the traffic-light gap) still works.
- Close tabs until few remain → no arrows, layout as before.

- [ ] **Step 6: Commit**

```bash
git add src/App.vue
git commit -m "feat(tabs): scroll the repo tab strip instead of wrapping"
```

---

### Task 10: Final checks

- [ ] **Step 1: Full build + tests**

Run: `npm run build && npm test && (cd src-tauri && cargo test 2>&1 | tail -3)`
Expected: build succeeds (vue-tsc + vite), vitest all green, cargo tests all green.

- [ ] **Step 2: Tick the spec's manual list**

Spec §6 manual list: `package-lock.json` + minified file in Changes (unified + split), commit detail, fullscreen; stage lines in virtual mode; Show anyway on a >20k-line diff; tab bar with ~15 repos in a narrow window. Anything not already covered in Tasks 8/9 QA (commit detail + fullscreen with a big file) — do it now.

- [ ] **Step 3: Push branch and open PR**

```bash
git push -u origin feat/large-diffs-tab-scroll
gh pr create --title "fix: large diffs no longer freeze; scrollable repo tabs" --body "$(cat <<'EOF'
## Summary
- Cap word-diff LCS table and per-line highlighting so a single minified line can't OOM or hang the renderer.
- Backend withholds diffs over 20k lines until the user clicks **Show anyway**.
- Diff rows are windowed above 1,500 lines — only the visible slice is rendered/highlighted.
- Repo tab bar never wraps: it scrolls horizontally, with ‹ › buttons that appear on overflow and wheel-to-scroll.

Spec: `docs/superpowers/specs/2026-09-18-large-diffs-tab-scroll-design.md`

## Test plan
- [ ] `npm test` (worddiff / highlight / diffrows guards)
- [ ] `cargo test` (`file_diff_gates_huge_diffs_unless_forced`)
- [ ] package-lock.json + minified bundle diffs in Changes, commit detail, fullscreen — unified and split
- [ ] Line staging in a windowed diff
- [ ] 15 open repos in a narrow window — one row, arrows, wheel scroll, active tab revealed

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```
