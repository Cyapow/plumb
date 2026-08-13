<script setup lang="ts">
import { ref, watch } from "vue";
import { stashApplyEx } from "../lib/git";
import { toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; index: number; label: string }>();
const emit = defineEmits<{ (e: "done"): void }>();

const pop = ref(false);
const restoreIndex = ref(false);
const busy = ref(false);

watch(open, (o) => {
  if (!o) return;
  pop.value = false;
  restoreIndex.value = false;
});

async function apply() {
  busy.value = true;
  try {
    await stashApplyEx(props.repoPath, props.index, pop.value, restoreIndex.value);
    toast(pop.value ? "Stash popped" : "Stash applied");
    open.value = false;
    emit("done");
  } catch (e) {
    toast("Apply failed", String(e), "error");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head"><h2>Apply stash</h2><button class="x" @click="open = false">✕</button></div>
        <div class="body">
          <p class="intro mono">{{ label }}</p>
          <label class="check"><input type="checkbox" v-model="pop" /> Delete the stash after applying <em>(pop)</em></label>
          <label class="check"><input type="checkbox" v-model="restoreIndex" /> Restore the staged state <em>(--index)</em></label>
          <div class="actions">
            <button class="btn-accent" :disabled="busy" @click="apply">Apply stash</button>
            <button class="btn" @click="open = false">Cancel</button>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 480px; max-width: calc(100vw - 48px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); }
.head { display: flex; align-items: center; padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); }
.intro { font-size: 12px; color: var(--text-mid); margin: 0 0 var(--space-4); }
.check { display: flex; align-items: center; gap: 8px; font-size: 12.5px; color: var(--text); margin-bottom: var(--space-2); cursor: pointer; }
.check em { color: var(--text-faint); font-style: normal; font-size: 11px; }
.actions { display: flex; gap: var(--space-2); margin-top: var(--space-4); }
.btn-accent { height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.6; }
.btn { height: 34px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
</style>
