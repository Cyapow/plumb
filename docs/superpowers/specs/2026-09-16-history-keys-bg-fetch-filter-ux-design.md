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

- Every 5 minutes, and when a repo becomes active (`loadRepo` or a cached-tab switch via `revalidateTab`), run a quiet fetch (`fetch_quiet` backend command: `git -c credential.interactive=false fetch --no-write-fetch-head --all --prune` with `GCM_INTERACTIVE=never`, `GIT_ASKPASS=true` so no credential UI can appear) for the active repo, throttled per repo path (skip if that repo was fetched < 5 min ago).
- Silent: no toast, no `syncing`/progress bar, errors swallowed (offline, no remotes, auth prompt failures). Skipped while `syncing` is true; a manual `sync()` marks itself syncing, then waits (at most 10 s) for any in-flight background fetch before starting, so the two never contend for remote ref locks.
- After a successful fetch, reload only `branches` (`listBranches`) if the active repo is unchanged and no manual sync started meanwhile; the `repo-changed` watcher handles the full refresh when remote refs actually moved.
- Paused while the window is hidden (`document.visibilityState !== "visible"`). On becoming visible, fetch immediately if the last background fetch was more than 5 minutes ago.
- One timer for the app, not per tab — only the active repo is fetched.

**UI**

- Pull button: `Pull ↓N` when `headInfo.behind > 0`; Push becomes `Push ↑N` so the pair reads symmetrically (matches the sidebar ↑↓).
- Sidebar ↑/↓ per branch already exists — unchanged.

**Code shape**

- `bgFetch()` async, guarded by `repo`, `syncing`, `bgFetching` and a per-path `lastBgFetch` map; exposes `bgFetchRun` so `sync()` can await it.
- `bgFetchTimer` alongside `ciPollTimer`; both cleared on unmount.
- `loadRepo()` and `revalidateTab()` call `bgFetch()` after their own loads, so opening a repo stays fast.

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

- Computed `filterActive` (trimmed) and `filterSummary` for the history strip; `clearCommitFilter()` (also called on tab switch and repo open); a `searchSeq` guard drops stale deep-search results; Esc in either input clears it. Sidebar mirrors with `sideFilterActive` and `clearSideFilter()`.
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
