<script setup lang="ts">
// The Plumb mark: a plumb line, an accent node, and the bob — the same glyph as
// the app icon. Line + bob use currentColor (so it adapts to context); the node
// is always the accent. Sized by `size` (height in px).
import { computed } from "vue";

const props = defineProps<{ size?: number }>();
const h = computed(() => props.size ?? 18);
const w = computed(() => (h.value * 32) / 90);
</script>

<template>
  <svg
    :width="w"
    :height="h"
    viewBox="0 0 32 90"
    fill="none"
    aria-hidden="true"
    style="display: block; flex: none"
  >
    <rect x="15" y="0" width="2" height="64" fill="currentColor" />
    <rect x="9.5" y="22" width="13" height="13" fill="var(--accent)" />
    <polygon points="0,64 32,64 16,90" fill="currentColor" />
  </svg>
</template>
