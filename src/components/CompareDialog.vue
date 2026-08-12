<script setup lang="ts">
// Pick two refs to compare; the resulting file list opens in the full-screen
// diff viewer (reused from commit/change diffs).
import { ref, watch } from "vue";
import { compareRefs, compareFileDiff } from "../lib/git";
import { openFullscreen, toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; branches: string[]; currentBranch: string | null; presetBase: string | null }>();

const base = ref("");
const compare = ref("");
const busy = ref(false);
const error = ref<string | null>(null);

watch(open, (o) => {
  if (!o) return;
  error.value = null;
  base.value = props.presetBase ?? ["main", "master"].find((b) => props.branches.includes(b)) ?? props.branches[0] ?? "";
  compare.value = props.currentBranch ?? props.branches.find((b) => b !== base.value) ?? "";
  if (compare.value === base.value) compare.value = props.branches.find((b) => b !== base.value) ?? compare.value;
});

async function run() {
  if (!base.value || !compare.value) return;
  if (base.value === compare.value) {
    error.value = "Pick two different refs.";
    return;
  }
  busy.value = true;
  error.value = null;
  try {
    const sum = await compareRefs(props.repoPath, base.value, compare.value);
    if (!sum.files.length) {
      toast("No differences", `${base.value} and ${compare.value} have identical trees.`);
      busy.value = false;
      return;
    }
    const b = base.value;
    const c = compare.value;
    openFullscreen({
      title: `${b} → ${c}`,
      subtitle: `${sum.ahead} ahead · ${sum.behind} behind · ${sum.files.length} file${sum.files.length === 1 ? "" : "s"}`,
      files: sum.files,
      load: (file) => compareFileDiff(props.repoPath, b, c, file),
    });
    open.value = false;
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head"><h2>Compare</h2><button class="x" @click="open = false">✕</button></div>
        <div class="body">
          <p class="intro">See what differs between two branches or commits.</p>
          <div class="row">
            <label class="field">
              <span>Base</span>
              <input v-model="base" list="cmp-branches" spellcheck="false" placeholder="main" />
            </label>
            <span class="arrow">→</span>
            <label class="field">
              <span>Compare</span>
              <input v-model="compare" list="cmp-branches" spellcheck="false" placeholder="feature/…" />
            </label>
            <datalist id="cmp-branches">
              <option v-for="b in branches" :key="b" :value="b" />
            </datalist>
          </div>
          <p class="hint mono">Tip: you can type a commit SHA or tag, not just a branch.</p>
          <p v-if="error" class="err mono">{{ error }}</p>
          <div class="actions">
            <button class="btn-accent" :disabled="busy" @click="run">{{ busy ? "Comparing…" : "Compare" }}</button>
            <button class="btn" @click="open = false">Cancel</button>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 560px; max-width: calc(100vw - 48px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); }
.head { display: flex; align-items: center; padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); }
.intro { font-size: 12.5px; color: var(--text-mid); margin: 0 0 var(--space-4); }
.row { display: flex; align-items: flex-end; gap: var(--space-3); }
.row .arrow { padding-bottom: 8px; color: var(--text-dim); }
.field { display: flex; flex-direction: column; gap: 4px; font-size: 11px; color: var(--text-dim); flex: 1; }
.field input { height: 34px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field input:focus { outline: none; border-color: var(--accent); }
.hint { font-size: 10.5px; color: var(--text-faint); margin: var(--space-2) 0 0; }
.err { color: var(--accent); font-size: 11px; margin: var(--space-3) 0 0; }
.actions { display: flex; gap: var(--space-2); margin-top: var(--space-4); }
.btn-accent { height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.6; }
.btn { height: 34px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
</style>
