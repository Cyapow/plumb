# History keyboard nav, background fetch, filter visibility — design

Three small UX tweaks to the main window (`src/App.vue`).

## 1. ↑/↓ cycles commits in History

**Behaviour**

- A `keydown` listener on `window`, registered in `onMounted`, removed in `onUnmounted`.
- Handles only `ArrowDown`, `ArrowUp`, `Escape`. Ignored unless all hold:
  - `view === "history"`
  - active element is not `input`, `textarea`, `select` or `contenteditable`
  - no modal dialog is open (command palette, confirm/input dialogs, etc. — check the existing `*Open` refs used for the dialogs)
- `ArrowDown`: select the commit after the current one in `visibleCommits` (so the filtered list is what you step through). Nothing selected → select the first row. At the last row → no-op, but trigger `loadMore()` so more history streams in.
- `ArrowUp`: select the previous commit. Nothing selected or first row → no-op.
- `Escape`: clear selection (closes the detail dock), only if something is selected.
- After a step, scroll the row into view. Rows are virtualised (`HIST_ROW_H = 34`), so scroll by index: keep the selected index inside `[scrollTop, scrollTop + clientHeight)` of `histBodyEl`, moving by the minimum needed (no re-centring).
- `preventDefault()` on handled keys so the page does not scroll as well.

**Code shape**

- Pure helper `stepIndex(current: number, delta: number, length: number): number | null` (returns null for no-op) kept next to the other list helpers so the clamping logic is obvious.
- `moveSelection(delta)` uses it, sets `selected`, then `ensureRowVisible(idx)`.

## 2. Background fetch + "behind" badge

**Behaviour**

- Every 5 minutes, and immediately when a repo is loaded or the active tab switches (`loadRepo`), run `git fetch --all --prune` for the active repo.
- Silent: no toast, no `syncing`/progress bar, errors swallowed (offline, no remotes, auth prompt failures). It must never block or race the manual buttons: skipped while `syncing` is true, and a manual `sync()` runs regardless of a background fetch in flight (git tolerates concurrent fetches; worst case one reports "already up to date").
- The existing `repo-changed` watcher picks up updated remote refs and refreshes branches, so `ahead`/`behind` update without extra plumbing. If the watcher does not fire for a ref-only change, call `refresh()` after a successful background fetch.
- Paused while the window is hidden (`document.visibilityState !== "visible"`). On becoming visible, fetch immediately if the last background fetch was more than 5 minutes ago.
- One timer for the app, not per tab — only the active repo is fetched.

**UI**

- Pull button: `Pull ↓N` when `headInfo.behind > 0`, mirroring the existing `Push N` for ahead. Push keeps its plain count; Pull uses the arrow because "behind" needs direction to read.
- Sidebar ↑/↓ per branch already exists — unchanged.

**Code shape**

- `bgFetch()` async, guarded by `repo`, `syncing`, `bgFetching` flags and a `lastBgFetch` timestamp.
- `bgFetchTimer` alongside `ciPollTimer`; both cleared on unmount.
- `loadRepo()` calls `bgFetch()` after the first paint (`nextTick` / after `loadCore`), not before, so opening a repo stays fast.

## 3. Make an active filter obvious

Two filters: the toolbar commit search (`commitFilter`) and the sidebar filter (`sideFilter`). Same pattern for both.

**Toolbar commit search (History view)**

- While `commitFilter` is non-empty, a strip between `.hist-head` and `.hist-body` (outside the scroll area, so it stays visible while scrolling):
  - `Filtering "foo" · 12 of 340 · Clear` for "In view" scope
  - `Searching all messages for "foo" · 12 results · Clear` / `Searching code in history for "foo" · 12 results · Clear` for the deep scopes; shows `Searching…` while `searching` is true.
  - "Clear" is a button that empties `commitFilter` (and `searchResults`).
- Empty result: rows area shows a centred `No commits match "foo"` with a Clear button instead of a blank list.
- The search input gets an accent border + subtle tinted background while non-empty (class `active`).

**Sidebar filter**

- While `sideFilter` is non-empty:
  - the input gets the same accent treatment
  - a one-line note directly under the input: `Showing matches for "foo" · Clear`
  - sections that would be hidden (Branches, Remotes, Stashes, Tags with no matches) render as a dim single line `Remotes · no matches` instead of disappearing, so the user sees the filter is why.
- No change to which items match; only presentation.

**Code shape**

- Computed `filterSummary` for the history strip text; the sidebar note is template-only.
- Styles: `.filter-strip`, `.filter-empty`, `.search.active`, `.side-filter.active`, `.sect-nomatch` — tokens from the existing palette (`--accent`, `--raised`, `--text-faint`).

## Out of scope

- Keyboard nav in the Changes file list or sidebar.
- Configurable fetch interval / per-remote background fetch.
- Auto-pull.

## Verification

No test runner in the repo. `npm run build` (vue-tsc) must pass, then manual check in the app:

1. History: click a commit, press ↓/↑, watch selection + detail dock follow; Esc closes; ↓ at the tail loads more; keys do nothing while typing in the search box.
2. Open a repo whose upstream has new commits: within seconds of opening, Pull shows `↓N`; leave it 5 minutes with a new remote commit and the badge updates.
3. Type in either filter: strip/note visible, accent border on input; a nonsense query shows the empty state / "no matches" section lines; Clear resets everything.
