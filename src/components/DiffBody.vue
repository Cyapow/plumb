<script setup lang="ts">
// Presentational diff renderer — blue/red, line-numbered. Optional per-hunk
// action button, and optional line selection for line-level staging.
import { computed, ref, watch } from "vue";
import type { DiffHunk, DiffLine } from "../lib/git";
import { highlightLine, langFromPath } from "../lib/highlight";
import { wordDiff, type Seg } from "../lib/worddiff";
import { prefs } from "../lib/ui";

const props = defineProps<{
  hunks: DiffHunk[];
  binary?: boolean;
  loading?: boolean;
  emptyText?: string;
  actionLabel?: string; // e.g. "Stage hunk" / "Unstage hunk"
  selectable?: boolean; // enable per-line selection
  filePath?: string | null; // for syntax highlighting
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

const emit = defineEmits<{
  (e: "hunkAction", index: number): void;
  (e: "lineAction", hunkIndex: number, lines: number[]): void;
}>();

const cls = (o: string) => (o === "+" ? "add" : o === "-" ? "del" : "ctx");
const verb = () => (props.actionLabel?.startsWith("Unstage") ? "Unstage" : "Stage");

// Side-by-side: pair deleted (left) with added (right) lines; context spans both.
interface Cell { l: DiffLine; li: number }
interface SplitRow { left?: Cell; right?: Cell; ctx?: boolean }
const split = computed(() => prefs.split);

// Side-by-side panes scroll horizontally on their own; keep their vertical
// scroll in lock-step so the two columns always show the same rows.
const leftPane = ref<HTMLElement | null>(null);
const rightPane = ref<HTMLElement | null>(null);
let syncing = false;
function onPaneScroll(from: "l" | "r") {
  if (syncing) return;
  const src = from === "l" ? leftPane.value : rightPane.value;
  const dst = from === "l" ? rightPane.value : leftPane.value;
  if (!src || !dst || dst.scrollTop === src.scrollTop) return;
  syncing = true;
  dst.scrollTop = src.scrollTop;
  requestAnimationFrame(() => (syncing = false));
}
const splitRows = computed<SplitRow[][]>(() =>
  props.hunks.map((h) => {
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
  }),
);

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
  <div class="diff">
    <div v-if="loading" class="empty">Reading diff…</div>
    <div v-else-if="binary" class="empty">Binary file — no textual diff.</div>
    <div v-else-if="hunks.length === 0" class="empty">{{ emptyText ?? "No changes to show." }}</div>
    <!-- Unified -->
    <div v-else-if="!split" class="hunks hunks-unified mono">
      <template v-for="(h, hi) in hunks" :key="hi">
        <div class="hunk-head">
          <span class="hh-text">{{ h.header }}</span>
          <span class="hunk-actions">
            <button
              v-if="selectable && pickedInHunk(hi).length"
              class="hunk-btn accent"
              @click="emit('lineAction', hi, pickedInHunk(hi))"
            >{{ verb() }} {{ pickedInHunk(hi).length }} line{{ pickedInHunk(hi).length === 1 ? "" : "s" }}</button>
            <button v-if="actionLabel" class="hunk-btn" @click="$emit('hunkAction', hi)">{{ actionLabel }}</button>
          </span>
        </div>
        <div
          v-for="(l, li) in h.lines"
          :key="hi + '-' + li"
          class="line"
          :class="[cls(l.origin), { picked: isPicked(hi, li), selectable: selectable && (l.origin === '+' || l.origin === '-') }]"
          @click="toggleLine(hi, li, l.origin)"
        >
          <span class="ln old">{{ l.old_lineno ?? "" }}</span>
          <span class="ln new">{{ l.new_lineno ?? "" }}</span>
          <span class="sign">{{ l.origin === " " ? "" : l.origin }}</span>
          <span v-if="segsFor(hi, li)" class="content"
            ><span v-for="(s, k) in segsFor(hi, li)" :key="k" :class="{ word: s.changed }">{{ s.text }}</span></span
          >
          <span v-else class="content" v-html="hl(l.content)"></span>
        </div>
      </template>
    </div>

    <!-- Side-by-side: two independent panes. Each scrolls horizontally on its
         own; their vertical scroll is kept in sync so rows stay aligned. -->
    <div v-else class="split-wrap">
      <div class="pane pane-left" ref="leftPane" @scroll="onPaneScroll('l')">
        <div class="pane-inner mono">
          <template v-for="(h, hi) in hunks" :key="hi">
            <div class="hunk-head">
              <span class="hh-text">{{ h.header }}</span>
              <span class="hunk-actions">
                <button v-if="actionLabel" class="hunk-btn" @click="$emit('hunkAction', hi)">{{ actionLabel }}</button>
              </span>
            </div>
            <div
              v-for="(row, ri) in splitRows[hi]"
              :key="hi + '-' + ri"
              class="pline"
              :class="row.ctx ? 'ctx' : row.left ? 'del' : 'empty'"
            >
              <span class="ln">{{ row.left?.l.old_lineno ?? "" }}</span>
              <span v-if="row.left && segsFor(hi, row.left.li)" class="content"
                ><span v-for="(s, k) in segsFor(hi, row.left.li)" :key="k" :class="{ word: s.changed }">{{ s.text }}</span></span
              >
              <span v-else-if="row.left" class="content" v-html="hl(row.left.l.content)"></span>
            </div>
          </template>
        </div>
      </div>
      <div class="pane pane-right" ref="rightPane" @scroll="onPaneScroll('r')">
        <div class="pane-inner mono">
          <template v-for="(h, hi) in hunks" :key="hi">
            <div class="hunk-head"><span class="hh-text">{{ h.header }}</span></div>
            <div
              v-for="(row, ri) in splitRows[hi]"
              :key="hi + '-' + ri"
              class="pline"
              :class="row.ctx ? 'ctx' : row.right ? 'add' : 'empty'"
            >
              <span class="ln">{{ row.right?.l.new_lineno ?? "" }}</span>
              <span v-if="row.right && segsFor(hi, row.right.li)" class="content"
                ><span v-for="(s, k) in segsFor(hi, row.right.li)" :key="k" :class="{ word: s.changed }">{{ s.text }}</span></span
              >
              <span v-else-if="row.right" class="content" v-html="hl(row.right.l.content)"></span>
            </div>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.diff { height: 100%; overflow: auto; background: var(--bg); }
.empty { padding: var(--space-6); color: var(--text-faint); font-size: 12.5px; }
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
