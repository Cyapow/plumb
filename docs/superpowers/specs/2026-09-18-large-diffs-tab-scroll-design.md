# Large diffs without crashing, scrollable tab bar — design

Two independent fixes.

- Opening a large changed file (`package-lock.json`, a minified bundle) hangs or crashes the app. Fix: guard the per-line work that explodes on long lines, gate huge diffs behind a "Show anyway" button, and virtualise the diff rows above a threshold.
- The open-repo tab bar wraps to a second row once enough repos are open. Fix: never wrap; scroll horizontally with `‹ ›` buttons.

## 1. Per-line crash guards (always on)

Root cause of the hard crash: `wordDiff` in `src/lib/worddiff.ts` builds an `(n+1)×(m+1)` LCS table for every paired `-`/`+` line. A minified line tokenises to tens of thousands of tokens, so the table is billions of cells → out of memory.

- `wordDiff(a, b)`: after tokenising, if `n === 0 || m === 0 || n * m > 250_000`, return `[[{ text: a, changed: true }], [{ text: b, changed: true }]]` — the whole line is marked changed, no LCS. Otherwise unchanged.
- `highlightLine(content, lang)` in `src/lib/highlight.ts`: if `content.length > 2000`, return `escapeHtml(content)` and skip highlight.js. Long lines are almost always minified/generated, where highlighting is noise anyway.

Both are pure functions and get unit tests.

## 2. Rust-side size gate

Diffs over `MAX_DIFF_LINES = 20_000` lines are not shipped over IPC unless asked for.

- `FileDiff` (`src-tauri/src/git.rs`) gains `truncated: bool` and `total_lines: usize`.
- `collect_file_diff(diff, file, staged, force)` first sums `patch.num_lines_in_hunk(h)` over all hunks. If `total_lines > MAX_DIFF_LINES && !force`, return with `hunks: []`, `truncated: true`. Otherwise collect lines as today (`truncated: false`).
- `file_diff`, `commit_file_diff`, `compare_file_diff` gain a trailing `force: Option<bool>` parameter (`None` → `false`) so existing invocations keep working.
- `src/lib/git.ts`: `FileDiff` type gets `truncated: boolean; total_lines: number`. `fileDiff`, `commitFileDiff`, `compareFileDiff` take an optional `force = false` and pass it through.

## 3. "Too big" placeholder and reload

- `DiffBody` gets props `truncated?: boolean`, `totalLines?: number` and emits `showAnyway`. When `truncated`, it renders, in the existing `.empty` style: "Large diff — 34,210 lines. Rendering may be slow." with a `Show anyway` button. Checked after `loading` and `binary`, before the hunks-empty case.
- `DiffView`: a `force` ref, reset to `false` whenever `file`, `staged` or `repoPath` change (not on `refresh`/`diffReloadKey`, so a stage/unstage doesn't re-collapse a diff you opted into). `showAnyway` sets `force = true` and reloads.
- `openFullscreen(...).load` becomes `(file: string, force?: boolean) => Promise<FileDiff>`. `DiffFullscreen` keeps the same `force` ref, reset on `activeFile` change. Callers in `ChangesView`, `CommitDetail`, `CompareDialog` forward `force`.

## 4. Virtualised rendering above 1,500 lines

Below 1,500 total lines the current templates are unchanged. Above it, `DiffBody` renders only the visible window of rows.

**Row model** — new pure module `src/lib/diffrows.ts`:

```ts
type UnifiedRow = { kind: "head"; hi: number } | { kind: "line"; hi: number; li: number; l: DiffLine };
type SplitRowItem = { kind: "head"; hi: number } | { kind: "row"; hi: number; ri: number; row: SplitRow };
flattenUnified(hunks): UnifiedRow[]
flattenSplit(hunks, splitRows): SplitRowItem[]
```

The existing `splitRows` computation moves into this module too (`buildSplitRows(hunks): SplitRow[][]`) so all row shaping is testable without the DOM.

**Windowing** — inside `DiffBody`:

- `virtual = computed(() => totalLines > 1500)`.
- `rowH` ref: measured from the first rendered row's `offsetHeight` after mount / rows change (line-height is `1.7 × 12px = 20.4px`, so measure rather than hardcode; fallback `20.4`).
- `viewStart`/`viewEnd` from the scroller's `scrollTop` and `clientHeight` with 20 rows of overscan. Updated on `scroll` (rAF-throttled) and via a `ResizeObserver` on the scroller.
- Rendered: a top spacer `div` of `viewStart * rowH`, the slice `rows.slice(viewStart, viewEnd)` with the same markup as the non-virtual path, a bottom spacer of `(rows.length - viewEnd) * rowH`.
- Unified: scroller is `.diff`. Split: scroller is the left pane; both panes render the same slice (they are already scroll-synced and share one row index space via `splitRows`).
- In virtual mode hunk headers are forced to the line height (`height: calc(var(--code-line-h) * 1em)`, buttons lose their vertical margin) so every row is the same height and the spacer maths stays exact.
- `wordSegs` stays eager (O(lines), cheap once guard §1 is in). Syntax highlighting now only runs for rendered rows — the main win.
- Line selection and hunk actions are keyed by `hi:li`, not DOM position, so they work unchanged in virtual mode.

## 5. Scrollable tab bar

`src/App.vue`, `.tabbar`.

**Markup**

```html
<div class="tabs-scroll" ref="tabsEl" @scroll="updateTabScroll" @wheel="onTabsWheel">
  <button v-for="t in tabs" ... class="repo-tab" :data-path="t.path">…</button>
</div>
<button v-show="tabsOverflow" class="tab-arrow" :disabled="!canL" @click="scrollTabs(-1)">‹</button>
<button v-show="tabsOverflow" class="tab-arrow" :disabled="!canR" @click="scrollTabs(1)">›</button>
<button class="add-tab" …>+</button>
```

**CSS**

- `.tabs-scroll { display: flex; align-items: stretch; flex: 0 1 auto; min-width: 0; overflow-x: auto; scrollbar-width: none; }` plus `.tabs-scroll::-webkit-scrollbar { display: none; }`.
- `.repo-tab { flex: none; }` so tabs keep their width and overflow instead of shrinking.
- `.tab-arrow` styled like `.add-tab` (same height, border-right, dim colour; `:disabled` fainter).

**Script**

- `tabsOverflow`, `canL`, `canR` refs. `updateTabScroll()`: `overflow = scrollWidth > clientWidth + 1`, `canL = scrollLeft > 0`, `canR = scrollLeft + clientWidth < scrollWidth - 1`.
- `ResizeObserver` on `.tabs-scroll` (set up in `onMounted`, disconnected in `onUnmounted`) and `watch(tabs, () => nextTick(updateTabScroll), { deep: true })`.
- `scrollTabs(dir)` → `tabsEl.scrollBy({ left: dir * 200, behavior: "smooth" })`.
- `onTabsWheel(e)`: if `Math.abs(e.deltaY) > Math.abs(e.deltaX)`, `scrollLeft += e.deltaY` and `preventDefault()`, so a plain mouse wheel scrolls the strip.
- After `selectTab`/opening a repo, `nextTick` → find `[data-path]` for the active tab → `scrollIntoView({ inline: "nearest", block: "nearest" })`.
- `startDrag` already ignores clicks on `button`, so the arrows and tabs behave; empty space in `.tabs-scroll` still drags the window.

## 6. Testing

- Add `vitest` as a devDependency with a `test` script. Tests:
  - `worddiff.test.ts`: normal pair still produces word segments; pair over the guard returns whole-line segments; empty side.
  - `highlight.test.ts`: short line highlighted; line over 2000 chars returned escaped.
  - `diffrows.test.ts`: `buildSplitRows` pairs dels/adds and spans context; `flattenUnified`/`flattenSplit` emit one head per hunk followed by its rows, in order.
- Manual: open `package-lock.json` and a minified file in Changes (unified + split), commit detail, fullscreen; stage lines in virtual mode; "Show anyway" on a >20k-line diff; tab bar with ~15 repos in a narrow window — arrows appear, wheel scrolls, active tab scrolled into view, no second row.
