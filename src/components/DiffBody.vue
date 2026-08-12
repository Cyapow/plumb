<script setup lang="ts">
// Presentational diff renderer — blue/red, line-numbered. Optional per-hunk
// action button, and optional line selection for line-level staging.
import { computed, ref, watch } from "vue";
import type { DiffHunk } from "../lib/git";
import { highlightLine, langFromPath } from "../lib/highlight";
import { wordDiff, type Seg } from "../lib/worddiff";

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
    <div v-else class="hunks mono">
      <template v-for="(h, hi) in hunks" :key="hi">
        <div class="hunk-head">
          <span class="hh-text">{{ h.header }}</span>
          <button
            v-if="selectable && pickedInHunk(hi).length"
            class="hunk-btn accent"
            @click="emit('lineAction', hi, pickedInHunk(hi))"
          >{{ verb() }} {{ pickedInHunk(hi).length }} line{{ pickedInHunk(hi).length === 1 ? "" : "s" }}</button>
          <button v-if="actionLabel" class="hunk-btn" @click="$emit('hunkAction', hi)">{{ actionLabel }}</button>
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
  </div>
</template>

<style scoped>
.diff { height: 100%; overflow: auto; background: var(--bg); }
.empty { padding: var(--space-6); color: var(--text-faint); font-size: 12.5px; }
.hunks { font-size: 12px; line-height: var(--diff-line-h); }
.hunk-head {
  display: flex; align-items: center; gap: var(--space-3);
  padding: 0 var(--space-2) 0 var(--space-4);
  background: var(--line-soft); color: var(--text-faint);
  border-top: 1px solid var(--raised); border-bottom: 1px solid var(--raised);
}
.hunk-head .hh-text { flex: 1; white-space: pre; overflow: hidden; text-overflow: ellipsis; }
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
</style>
