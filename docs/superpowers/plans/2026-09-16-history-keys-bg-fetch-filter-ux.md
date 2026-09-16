# History keys, background fetch, filter visibility — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ↑/↓ steps through commits in History; the active repo is fetched in the background so Pull shows `↓N`; both filters make it obvious they are active.

**Architecture:** All three changes live in `src/App.vue` (the single-file main window — the codebase keeps app state there, so we follow it). Each is an isolated block: a keydown handler + two helpers; a background-fetch timer next to the existing CI poll; template/CSS additions for the two filters. No backend changes — `git fetch --all --prune` already exists as the `fetch` command and `ahead/behind` already come back from `listBranches`.

**Tech Stack:** Vue 3 `<script setup>` + TypeScript, Tauri 2 backend (Rust, untouched). No test runner in the repo: each task is verified by `npm run build` (runs `vue-tsc --noEmit`) plus a manual check in the app (`npm run tauri dev`).

Spec: `docs/superpowers/specs/2026-09-16-history-keys-bg-fetch-filter-ux-design.md`

---

## File map

- Modify: `src/App.vue` — only file touched.
  - script: keyboard nav (after `measureHist`, ~line 185), background fetch (next to `ciPollTimer`, ~line 1275), `filterSummary` computed (after `onScopeChange`, ~line 610).
  - template: Pull button (~line 1692), search input (~line 1717), history list (~line 1969), sidebar filter + sections (~line 1809–1920).
  - style: `.search.active`, `.filter-strip`, `.filter-empty`, `.side-filter.active`, `.side-note`, `.sect-nomatch`.

Line numbers are from the branch at commit `7105846`; grep for the quoted anchors rather than trusting them.

---

### Task 1: ↑/↓/Esc keyboard navigation in History

**Files:**
- Modify: `src/App.vue` (script after `measureHist`; `onMounted`/`onUnmounted`)

- [ ] **Step 1: Add the pure step helper and selection mover**

Insert directly after the `measureHist` function (anchor: `function measureHist() {`):

```ts
/**
 * Index to move to when stepping a selection by `delta` in a list of `length`.
 * Returns null when the move is a no-op (empty list, or already at the edge).
 * `current` is -1 when nothing is selected: ↓ then picks the first row.
 */
function stepIndex(current: number, delta: number, length: number): number | null {
  if (length === 0) return null;
  if (current < 0) return delta > 0 ? 0 : null;
  const next = current + delta;
  if (next < 0 || next >= length) return null;
  return next;
}

/** Scroll the virtualised history so row `idx` is inside the viewport (minimal move). */
function ensureRowVisible(idx: number) {
  const el = histBodyEl.value;
  if (!el) return;
  const top = idx * HIST_ROW_H;
  const bottom = top + HIST_ROW_H;
  if (top < el.scrollTop) el.scrollTop = top;
  else if (bottom > el.scrollTop + el.clientHeight) el.scrollTop = bottom - el.clientHeight;
}

/** ↑/↓ in History: step the selection through the visible (filtered) commits. */
function moveSelection(delta: 1 | -1) {
  const list = visibleCommits.value;
  const cur = selected.value ? list.findIndex((c) => c.id === selected.value) : -1;
  const next = stepIndex(cur, delta, list.length);
  if (next === null) {
    // At the tail: pull the next page in so the user can keep going.
    if (delta > 0 && cur >= 0) loadMoreCommits();
    return;
  }
  selected.value = list[next].id;
  ensureRowVisible(next);
}

/** True when a text field or a modal has focus, so list keys must stay out of the way. */
function keyboardBusy(): boolean {
  const a = document.activeElement as HTMLElement | null;
  if (a && (a.tagName === "INPUT" || a.tagName === "TEXTAREA" || a.tagName === "SELECT" || a.isContentEditable)) return true;
  if (paletteOpen.value) return true;
  return !!document.querySelector(".backdrop, .pal-backdrop");
}

function onHistoryKey(e: KeyboardEvent) {
  if (view.value !== "history" || !repo.value || keyboardBusy()) return;
  if (e.key === "ArrowDown") {
    e.preventDefault();
    moveSelection(1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    moveSelection(-1);
  } else if (e.key === "Escape" && selected.value) {
    e.preventDefault();
    selected.value = null;
  }
}
```

Note: `visibleCommits`, `selected`, `paletteOpen`, `view`, `repo` are declared later in the file but these are functions, so referencing them is fine at call time. `loadMoreCommits` bails out itself when a filter is active or everything is loaded.

- [ ] **Step 2: Register / unregister the listener**

In `onMounted` (anchor: `refreshConnections();` at the top of the `onMounted` body) add as the first line:

```ts
  window.addEventListener("keydown", onHistoryKey);
```

In `onUnmounted` (anchor: `if (ciPollTimer) clearInterval(ciPollTimer);`) add after it:

```ts
  window.removeEventListener("keydown", onHistoryKey);
```

- [ ] **Step 3: Type-check**

Run: `npm run build`
Expected: completes with no `vue-tsc` errors (vite build output follows).

- [ ] **Step 4: Manual check**

Run: `npm run tauri dev`, open a repo with 200+ commits, History view.
- Click a commit → ↓ selects the next, detail dock follows; ↑ goes back; ↑ on the first row does nothing.
- Click the search box, press ↓ → selection does not move (input focused).
- Press Esc with a commit selected → dock closes.
- Scroll to the bottom, hold ↓ → more commits stream in and selection keeps advancing.
- Type a filter, ↓ steps only through the matching rows.

- [ ] **Step 5: Commit**

```bash
git add src/App.vue
git commit -m "feat(history): step through commits with arrow keys, Esc closes detail"
```

---

### Task 2: Background fetch every 5 min + on repo load; Pull `↓N` badge

**Files:**
- Modify: `src/App.vue` (script near `ciPollTimer`; `loadRepo`; Pull button template)

- [ ] **Step 1: Add the background fetch block**

Insert directly before `let ciPollTimer: number | undefined;`:

```ts
/* ── Background fetch ─────────────────────────────────────────────── */
// Fetch the active repo quietly every few minutes (and on open) so the
// Pull button can show how far behind upstream we are. Silent by design:
// no toast, no spinner, failures ignored (offline, no remotes, auth).
const BG_FETCH_MS = 5 * 60_000;
let bgFetchTimer: number | undefined;
let bgFetching = false;
let lastBgFetch = 0;
async function bgFetch(force = false) {
  if (!repo.value || syncing.value || bgFetching) return;
  if (document.visibilityState !== "visible") return;
  if (!force && Date.now() - lastBgFetch < BG_FETCH_MS) return;
  bgFetching = true;
  const path = repo.value.path;
  try {
    await gitFetch(path);
    lastBgFetch = Date.now();
    // The repo watcher usually notices the ref update; refresh anyway in case
    // it was a no-op fetch or the watcher was quiet, so ahead/behind is current.
    if (repo.value?.path === path) await refresh();
  } catch {
    /* quiet: try again next tick */
  } finally {
    bgFetching = false;
  }
}
function onVisibility() {
  if (document.visibilityState === "visible") bgFetch();
}
```

- [ ] **Step 2: Start the timer, fetch on visibility, clean up**

In `onMounted`, directly after the existing CI poll block (anchor: `}, 90_000);`) add:

```ts
  bgFetchTimer = window.setInterval(() => bgFetch(), BG_FETCH_MS);
  document.addEventListener("visibilitychange", onVisibility);
```

In `onUnmounted`, after `if (ciPollTimer) clearInterval(ciPollTimer);` add:

```ts
  if (bgFetchTimer) clearInterval(bgFetchTimer);
  document.removeEventListener("visibilitychange", onVisibility);
```

- [ ] **Step 3: Fetch on repo open / tab switch**

In `loadRepo`, directly after `if (!allCommitsLoaded.value) void loadMoreCommits();` add:

```ts
    // Quiet fetch after first paint so the behind-count is fresh for this repo.
    void bgFetch(true);
```

- [ ] **Step 4: Pull button badge**

Replace the Pull button (anchor: `@click="doPull" @contextmenu="pullMenu"`):

```html
          <button class="btn" :disabled="syncing" @click="doPull" @contextmenu="pullMenu" title="Pull · right-click for rebase / ff-only">
            Pull<span v-if="headInfo && headInfo.behind"> ↓{{ headInfo.behind }}</span>
            <kbd>⇧⌘P</kbd>
          </button>
```

- [ ] **Step 5: Type-check**

Run: `npm run build`
Expected: no errors.

- [ ] **Step 6: Manual check**

Run: `npm run tauri dev`.
- Open a repo whose upstream has commits you don't have (e.g. `git reset --hard HEAD~2` in a clone). Within a few seconds Pull reads `Pull ↓2`; no toast, no progress bar.
- Push a commit to the remote from another clone, wait ≤5 min → badge increments. Or temporarily set `BG_FETCH_MS = 20_000` to observe, then restore.
- Hide the window (⌘H) for a minute, show it → a fetch runs if stale (watch `git reflog show refs/remotes/origin/main` or the badge).
- Click Fetch while a background fetch is running → still works, one toast.

- [ ] **Step 7: Commit**

```bash
git add src/App.vue
git commit -m "feat(sync): background fetch every 5 min and on open; Pull shows behind count"
```

---

### Task 3: History search — active strip, empty state, input accent

**Files:**
- Modify: `src/App.vue` (script after `onScopeChange`; search input; history list template; styles)

- [ ] **Step 1: Add the summary computed and a clear helper**

Insert directly after the `onScopeChange` function:

```ts
/** One-line description of the active commit search, for the strip above the list. */
const filterSummary = computed(() => {
  const q = commitFilter.value.trim();
  if (!q) return "";
  const n = visibleCommits.value.length;
  if (searchScope.value === "view") return `Filtering "${q}" · ${n} of ${commits.value.length}`;
  const what = searchScope.value === "code" ? "code in history" : "all messages";
  if (searching.value) return `Searching ${what} for "${q}"…`;
  return `Searched ${what} for "${q}" · ${n} result${n === 1 ? "" : "s"}`;
});
function clearCommitFilter() {
  commitFilter.value = "";
  searchResults.value = [];
}
```

- [ ] **Step 2: Accent the search box while active; use the clear helper**

Replace the opening `<div class="search">` with:

```html
      <div class="search" :class="{ active: commitFilter }">
```

Replace the clear glyph line (anchor: `class="clear" title="Clear" @click="commitFilter = ''"`):

```html
        <span v-if="commitFilter" class="clear" title="Clear" @click="clearCommitFilter">✕</span>
```

- [ ] **Step 3: Strip above the list, empty state inside it**

Directly after the closing `</div>` of `.hist-head` (the line after `<button class="cols-btn" … >⋯</button>` and its `</div>`), insert:

```html
          <div v-if="commitFilter" class="filter-strip">
            <span class="fs-text">{{ filterSummary }}</span>
            <button class="fs-clear" @click="clearCommitFilter">Clear</button>
          </div>
```

Directly after `<div ref="histBodyEl" class="hist-body" @scroll="onHistScroll">` insert:

```html
            <div v-if="commitFilter && !searching && !visibleCommits.length" class="filter-empty">
              <div class="fe-title">No commits match "{{ commitFilter.trim() }}"</div>
              <div class="fe-sub">
                {{ searchScope === "view" ? "Only loaded commits are searched in this scope — try “All · message” or “All · code”." : "Nothing in this repository's history matched." }}
              </div>
              <button class="btn" @click="clearCommitFilter">Clear search</button>
            </div>
```

- [ ] **Step 4: Styles**

After `.search .clear { … }` add:

```css
.search.active { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 8%, var(--bg)); }
.search.active .glyph { color: var(--accent); }
```

After `.hist-body { position: relative; flex: 1; overflow-y: auto; }` add:

```css
.filter-strip {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--space-3);
  height: 26px;
  padding: 0 var(--space-3);
  background: color-mix(in srgb, var(--accent) 10%, var(--bg));
  border-bottom: 1px solid var(--accent);
  color: var(--text-mid);
  font-size: 11.5px;
}
.filter-strip .fs-text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.filter-strip .fs-clear { flex: none; height: 18px; padding: 0 8px; background: var(--raised); border: 1px solid var(--line); color: var(--text); font-size: 10.5px; cursor: pointer; }
.filter-strip .fs-clear:hover { border-color: var(--accent); }
.filter-empty { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--space-2); padding: var(--space-6); text-align: center; z-index: 2; }
.filter-empty .fe-title { font-size: 13px; color: var(--text); }
.filter-empty .fe-sub { font-size: 11.5px; color: var(--text-faint); max-width: 380px; line-height: 1.45; }
```

- [ ] **Step 5: Type-check**

Run: `npm run build`
Expected: no errors.

- [ ] **Step 6: Manual check**

- Type `fix` in the search: input border turns accent, strip reads `Filtering "fix" · N of M`, graph hidden as before.
- Switch scope to `All · message`: strip shows `Searching…` briefly then `Searched all messages for "fix" · N results`.
- Type `zzzzqq`: list area shows the empty state with the Clear button; click it → everything resets, graph returns.
- Strip stays put while scrolling the list.

- [ ] **Step 7: Commit**

```bash
git add src/App.vue
git commit -m "feat(search): show an active-filter strip and a no-match state in History"
```

---

### Task 4: Sidebar filter — note, no-match section lines, input accent

**Files:**
- Modify: `src/App.vue` (sidebar template; styles)

- [ ] **Step 1: Accent + note under the filter input**

Replace the sidebar filter block (anchor: `<div class="side-filter">`) with:

```html
        <div class="side-filter" :class="{ active: sideFilter }">
          <span class="sf-ico">⌕</span>
          <input v-model="sideFilter" placeholder="Filter branches, tags, stashes…" spellcheck="false" />
          <button v-if="sideFilter" class="sf-x" title="Clear" @click="sideFilter = ''">✕</button>
        </div>
        <div v-if="sideFilter" class="side-note">
          Showing matches for "{{ sideFilter.trim() }}" · <button class="sn-clear" @click="sideFilter = ''">Clear</button>
        </div>
```

- [ ] **Step 2: Keep hidden sections visible as "no matches" lines**

Branches — change `<nav class="side-section" v-if="localTree.length">` to:

```html
        <nav class="side-section" v-if="localTree.length || (sideFilter && localBranches.length)">
```
and inside it, directly after the `sect-head` div, wrap the existing body so it only renders when there are matches:

```html
          <div v-if="!localTree.length" class="sect-nomatch">no matches</div>
          <template v-else-if="!collapsedSections.branches">
```
(the existing `<template v-if="!collapsedSections.branches">` becomes that `v-else-if`; its contents and closing tag stay).

Remotes — change the nav condition to:

```html
        <nav class="side-section" v-if="remoteTree.length || remotes.length">
```
and directly after its `sect-head` div add:

```html
          <div v-if="sideFilter && !remoteTree.length" class="sect-nomatch">no matches</div>
          <template v-else-if="!collapsedSections.remotes">
```
(again the existing `<template v-if="!collapsedSections.remotes">` becomes this `v-else-if`).

Stashes — change the nav condition to `v-if="!sideFilter || stashes.length"` and directly after its `sect-head` add:

```html
          <div v-if="sideFilter && !fStashes.length" class="sect-nomatch">no matches</div>
          <template v-else-if="!collapsedSections.stashes">
```

Tags — change `v-if="fTags.length"` to `v-if="fTags.length || (sideFilter && tags.length)"` and directly after its `sect-head` add:

```html
          <div v-if="!fTags.length" class="sect-nomatch">no matches</div>
          <template v-else-if="!collapsedSections.tags">
```

Result: a section only disappears when the repo has nothing of that kind at all; under a filter with zero hits it shows its header plus a dim `no matches` line.

- [ ] **Step 3: Styles**

After `.side-filter .sf-x { … }` add:

```css
.side-filter.active { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 8%, var(--bg)); }
.side-filter.active .sf-ico { color: var(--accent); }
.side-note { margin: var(--space-1) var(--space-3) 0; font-size: 10.5px; color: var(--text-faint); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.side-note .sn-clear { background: none; border: none; padding: 0; color: var(--accent); font-size: inherit; cursor: pointer; }
.sect-nomatch { padding: 0 var(--space-3) var(--space-2) 22px; font-size: 11px; color: var(--text-faint); font-style: italic; }
```

- [ ] **Step 4: Type-check**

Run: `npm run build`
Expected: no errors.

- [ ] **Step 5: Manual check**

- Type `main` in the sidebar filter: input accent, note `Showing matches for "main" · Clear`, Branches/Remotes show matches, Stashes and Tags show `no matches` under their headers (if the repo has any).
- Type `zzzz`: every section shows `no matches`; nothing vanishes.
- Click Clear in the note → full sidebar returns.
- Clear the filter in a repo with no stashes → Stashes section still shows `No stashes.` as before.

- [ ] **Step 6: Commit**

```bash
git add src/App.vue
git commit -m "feat(sidebar): make the active filter obvious instead of hiding sections"
```

---

### Task 5: Final verification

- [ ] **Step 1: Full build**

Run: `npm run build`
Expected: clean.

- [ ] **Step 2: Walk the spec's verification list** (spec §Verification, items 1–3) once more end-to-end in the app.

- [ ] **Step 3: Finish the branch** — use the superpowers:finishing-a-development-branch skill.
