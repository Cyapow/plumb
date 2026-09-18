<script setup lang="ts">
// Presentational diff renderer — blue/red, line-numbered. Optional per-hunk
// action button, and optional line selection for line-level staging.
//
// Above VIRTUAL_MIN_LINES only the rows near the viewport are rendered (row
// windowing): a lockfile diff of 30k lines would otherwise mean 30k DOM rows
// and 30k highlight.js calls up front.
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import type { DiffHunk, DiffLine } from "../lib/git";
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

// Word-level diff for 1:1 changed line pairs (a lone "-" followed by a lone
// "+"). Computed on demand per rendered row and memoised, so a windowed diff
// only pays for the lines on screen.
const segCache = new Map<string, Seg[] | null>();
watch(() => props.hunks, () => segCache.clear());

// A lone "-" at `di` immediately followed by a lone "+".
function isLonePair(lines: DiffLine[], di: number): boolean {
  return (
    lines[di]?.origin === "-" &&
    lines[di + 1]?.origin === "+" &&
    lines[di + 2]?.origin !== "+" &&
    (di === 0 || lines[di - 1].origin !== "-")
  );
}
function segsFor(hi: number, li: number): Seg[] | undefined {
  const k = `${hi}:${li}`;
  const hit = segCache.get(k);
  if (hit !== undefined) return hit ?? undefined;
  const lines = props.hunks[hi]?.lines ?? [];
  const origin = lines[li]?.origin;
  let del = -1;
  if (origin === "-" && isLonePair(lines, li)) del = li;
  else if (origin === "+" && li > 0 && isLonePair(lines, li - 1)) del = li - 1;
  if (del < 0) {
    segCache.set(k, null);
    return undefined;
  }
  const [d, a] = wordDiff(lines[del].content, lines[del + 1].content);
  segCache.set(`${hi}:${del}`, d);
  segCache.set(`${hi}:${del + 1}`, a);
  return del === li ? d : a;
}

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

<style scoped>
.diff { height: 100%; overflow: auto; background: var(--bg); }
.empty { padding: var(--space-6); color: var(--text-faint); font-size: 12.5px; }
.empty .show-anyway { margin-top: var(--space-3); }
.hunks { font-family: var(--code-font); font-size: var(--code-font-size); line-height: var(--code-line-h); }
/* Size to the widest line so row backgrounds and the hunk bar span the full
   horizontal scroll width, not just the viewport. min-width keeps it full-bleed
   when the content is narrower than the pane. */
.hunks-unified { width: max-content; min-width: 100%; }
.hunk-head {
  display: flex; align-items: center; gap: var(--space-3);
  padding: 0 var(--space-2) 0 var(--space-4);
  background: var(--line-soft); color: var(--text-faint);
  border-top: 1px solid var(--raised); border-bottom: 1px solid var(--raised);
}
.hunk-head .hh-text { flex: 1; white-space: pre; overflow: hidden; text-overflow: ellipsis; }
/* Keep the stage/unstage buttons pinned to the right edge of the pane so a
   horizontal scroll can't push them out of reach. */
.hunk-actions {
  position: sticky; right: 0; margin-left: auto;
  display: flex; align-items: center; gap: 6px;
  padding-left: 10px; background: var(--line-soft);
}
.hunk-actions:empty { display: none; }
.hunk-btn { flex: none; font-family: var(--font-ui); font-size: 10.5px; font-weight: 600; padding: 2px 8px; margin: 3px 0; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); cursor: pointer; }
.hunk-btn:hover { border-color: var(--accent); color: var(--accent); }
.hunk-btn.accent { background: var(--accent); color: var(--accent-on); border-color: var(--accent); }
/* Windowed mode assumes every row is exactly one line tall so the spacer
   maths is exact: hunk headers lose the button's vertical margin. */
.spacer { flex: none; }
.virtual .hunk-head { height: calc(var(--code-line-h) * 1em); overflow: hidden; }
.virtual .hunk-btn { margin: 0; padding: 0 8px; line-height: 16px; }
.line { display: flex; }
.line.selectable { cursor: pointer; }
.line.selectable:hover { filter: brightness(1.15); }
.line.picked { box-shadow: inset 3px 0 0 var(--accent); }
.line .ln { width: 44px; flex: none; text-align: right; padding-right: var(--space-3); color: var(--text-faint); user-select: none; }
.line .sign { width: 14px; flex: none; text-align: center; }
/* Background tint marks add/remove; syntax highlighting colors the code. */
.line .content { white-space: pre; flex: 1; user-select: text; color: var(--text); }
.line.add { background: var(--diff-add-bg); }
.line.add .sign { color: var(--diff-add-fg); }
.line.add .ln.new { color: var(--diff-add-num); }
.line.del { background: var(--diff-del-bg); }
.line.del .sign { color: var(--diff-del-fg); }
/* Word-level emphasis within a changed line. */
.line.add .word { background: color-mix(in srgb, var(--diff-add-num) 32%, transparent); }
.line.del .word { background: color-mix(in srgb, var(--diff-del-fg) 32%, transparent); }

/* ── Side-by-side ──
   Two panes, each its own horizontal scroller (a scrollbar per pane, pinned to
   that pane's bottom), with vertical scroll synced in script so rows align. */
.split-wrap { display: flex; height: 100%; min-height: 0; }
.pane { flex: 1 1 50%; min-width: 0; overflow: auto; }
.pane.pane-right { border-left: 1px solid var(--line-soft); }
.pane-inner {
  width: max-content; min-width: 100%;
  font-family: var(--code-font); font-size: var(--code-font-size); line-height: var(--code-line-h);
}
.pline { display: flex; }
.pline .ln { width: 44px; flex: none; text-align: right; padding-right: var(--space-3); color: var(--text-faint); user-select: none; }
.pline .content { white-space: pre; flex: 1; user-select: text; color: var(--text); padding-right: var(--space-3); }
.pline.add { background: var(--diff-add-bg); }
.pline.add .ln { color: var(--diff-add-num); }
.pline.del { background: var(--diff-del-bg); }
.pline.empty { background: color-mix(in srgb, var(--line-soft) 40%, transparent); }
.pline.add .word { background: color-mix(in srgb, var(--diff-add-num) 32%, transparent); }
.pline.del .word { background: color-mix(in srgb, var(--diff-del-fg) 32%, transparent); }
</style>
