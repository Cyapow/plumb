<script setup lang="ts">
// Recursive branch tree. Folders collapse; leaves behave like the old branch
// rows (click = jump to tip, double-click = check out, right-click = menu).
import { inject, ref } from "vue";
import type { BranchNode } from "../lib/branchtree";
import type { BranchInfo } from "../lib/git";

defineProps<{ nodes: BranchNode[]; depth?: number }>();

interface Actions {
  checkout: (name: string) => void;
  jump: (target: string | null) => void;
  menu: (e: MouseEvent, b: BranchInfo) => void;
  colorFor: (name: string) => string;
}
const actions = inject<Actions>("branchActions")!;
const collapsed = ref<Record<string, boolean>>({});
const pad = (depth: number) => `${(depth ?? 0) * 14 + 12}px`;
</script>

<template>
  <template v-for="node in nodes" :key="node.path">
    <div
      v-if="node.branch"
      class="row leaf mono"
      :class="{ head: node.branch.is_head }"
      :style="{ paddingLeft: pad(depth ?? 0) }"
      :title="node.branch.is_head ? 'Current branch' : `Click to jump to tip · double-click to check out ${node.branch.name}`"
      @click="actions.jump(node.branch.target)"
      @dblclick="!node.branch.is_head && actions.checkout(node.branch.name)"
      @contextmenu="actions.menu($event, node.branch)"
    >
      <span class="dot" :style="{ background: actions.colorFor(node.branch.name) }"></span>
      <span class="ellipsis">{{ node.name }}</span>
      <span v-if="node.branch.is_head" class="head-tag mono">HEAD</span>
    </div>
    <template v-else>
      <div class="row folder" :style="{ paddingLeft: pad(depth ?? 0) }" @click="collapsed[node.path] = !collapsed[node.path]">
        <span class="chev">{{ collapsed[node.path] ? "▸" : "▾" }}</span>
        <span class="ellipsis">{{ node.name }}</span>
      </div>
      <BranchTree v-if="!collapsed[node.path]" :nodes="node.children" :depth="(depth ?? 0) + 1" />
    </template>
  </template>
</template>

<style scoped>
.row {
  height: 26px;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding-right: var(--space-3);
  font-size: 12px;
  color: var(--text-mid);
  cursor: pointer;
}
.row:hover { background: color-mix(in srgb, var(--raised) 55%, transparent); }
.row.leaf.head { color: var(--text); font-weight: 500; }
.row .ellipsis { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.dot { width: 8px; height: 8px; flex: none; }
.chev { width: 10px; flex: none; color: var(--text-faint); font-size: 9px; }
.folder { color: var(--text-dim); }
.head-tag { margin-left: auto; font-size: 9.5px; font-weight: 700; background: var(--accent); color: var(--accent-on); padding: 1px 4px; flex: none; }
</style>
