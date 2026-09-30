<script setup lang="ts">
// Renders just the graph column: lane segments + a node per commit row.
// Node shape encodes kind (design rule "hue plus form"):
//   filled square = commit · hollow square = merge · ringed = HEAD.
//
// The SVG is full-height and sits absolutely inside the history scroller, so it
// scrolls in lockstep with the commit rows — no separate scroll context. It
// emits its pixel width so the list can reserve a matching text gutter.
//
// Layout covers every loaded commit (lanes depend on all rows above), but only
// the rows in [from, to) — the list's virtual window — are put in the DOM, so
// thousands of loaded commits don't mean tens of thousands of SVG elements.
import { computed, watch } from "vue";
import type { CommitRow } from "../lib/git";
import { layoutGraph, NODE_R, LANE_W } from "../lib/graph";

const props = defineProps<{ commits: CommitRow[]; from?: number; to?: number }>();
const emit = defineEmits<{ (e: "width", w: number): void }>();

const layout = computed(() => layoutGraph(props.commits));
const range = computed(() => {
  const n = layout.value.nodes.length;
  const from = Math.max(0, Math.min(n, props.from ?? 0));
  const to = Math.max(from, Math.min(n, props.to ?? n));
  return { from, to };
});
const segments = computed(() => {
  const { from, to } = range.value;
  const rs = layout.value.rowSeg;
  return layout.value.segments.slice(rs[from] ?? 0, rs[to] ?? 0);
});
const nodes = computed(() => layout.value.nodes.slice(range.value.from, range.value.to));
const laneVar = (lane: number) => `var(--lane-${lane})`;

watch(() => layout.value.width, (w) => emit("width", w), { immediate: true });
</script>

<template>
  <svg
    class="graph"
    :width="Math.max(layout.width, LANE_W)"
    :height="layout.height"
    :viewBox="`0 0 ${Math.max(layout.width, LANE_W)} ${layout.height}`"
    aria-hidden="true"
  >
    <line
      v-for="(s, i) in segments"
      :key="'s' + ((layout.rowSeg[range.from] ?? 0) + i)"
      :x1="s.x1"
      :y1="s.y1"
      :x2="s.x2"
      :y2="s.y2"
      :stroke="laneVar(s.lane)"
      stroke-width="2"
      fill="none"
    />
    <template v-for="(n, i) in nodes" :key="'n' + (range.from + i)">
      <!-- HEAD: ringed square -->
      <rect
        v-if="n.head"
        :x="n.col * LANE_W + LANE_W / 2 - (NODE_R + 2)"
        :y="n.y - (NODE_R + 2)"
        :width="(NODE_R + 2) * 2"
        :height="(NODE_R + 2) * 2"
        fill="none"
        :stroke="laneVar(n.lane)"
        stroke-width="2"
      />
      <!-- merge: hollow square · commit: filled square -->
      <rect
        :x="n.col * LANE_W + LANE_W / 2 - NODE_R"
        :y="n.y - NODE_R"
        :width="NODE_R * 2"
        :height="NODE_R * 2"
        :fill="n.merge && !n.head ? 'var(--bg)' : laneVar(n.lane)"
        :stroke="laneVar(n.lane)"
        stroke-width="2"
      />
    </template>
  </svg>
</template>

<style scoped>
.graph {
  display: block;
  pointer-events: none;
  overflow: visible;
}
</style>
