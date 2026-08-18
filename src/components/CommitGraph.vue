<script setup lang="ts">
// Renders just the graph column: lane segments + a node per commit row.
// Node shape encodes kind (design rule "hue plus form"):
//   filled square = commit · hollow square = merge · ringed = HEAD.
//
// The SVG is full-height and sits absolutely inside the history scroller, so it
// scrolls in lockstep with the commit rows — no separate scroll context. It
// emits its pixel width so the list can reserve a matching text gutter.
import { computed, watch } from "vue";
import type { CommitRow } from "../lib/git";
import { layoutGraph, NODE_R, LANE_W } from "../lib/graph";

const props = defineProps<{ commits: CommitRow[] }>();
const emit = defineEmits<{ (e: "width", w: number): void }>();

const layout = computed(() => layoutGraph(props.commits));
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
      v-for="(s, i) in layout.segments"
      :key="'s' + i"
      :x1="s.x1"
      :y1="s.y1"
      :x2="s.x2"
      :y2="s.y2"
      :stroke="laneVar(s.lane)"
      stroke-width="2"
      fill="none"
    />
    <template v-for="(n, i) in layout.nodes" :key="'n' + i">
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
