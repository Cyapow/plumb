<script setup lang="ts">
// Guided merge / rebase — pick a branch and set options. The parent runs the
// operation (so conflict handling flows through the in-progress banner).
import { ref, watch } from "vue";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{
  mode: "merge" | "rebase";
  branches: string[];
  currentBranch: string | null;
  presetBranch: string | null;
}>();
const emit = defineEmits<{
  (e: "confirm", mode: "merge" | "rebase", branch: string, opts: Record<string, boolean>): void;
}>();

const branch = ref("");
// merge
const squash = ref(false);
const noFf = ref(false);
const noCommit = ref(false);
const verifySignatures = ref(false);
// shared
const noVerify = ref(false);
// rebase
const autostash = ref(true);

watch(open, (o) => {
  if (!o) return;
  branch.value = props.presetBranch ?? props.branches.find((b) => b !== props.currentBranch) ?? props.branches[0] ?? "";
  squash.value = false;
  noFf.value = false;
  noCommit.value = false;
  verifySignatures.value = false;
  noVerify.value = false;
  autostash.value = true;
});

function run() {
  if (!branch.value) return;
  const opts: Record<string, boolean> =
    props.mode === "merge"
      ? { squash: squash.value, noFf: noFf.value, noCommit: noCommit.value, verifySignatures: verifySignatures.value, noVerify: noVerify.value }
      : { autostash: autostash.value, noVerify: noVerify.value };
  emit("confirm", props.mode, branch.value, opts);
  open.value = false;
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head"><h2>{{ mode === "merge" ? "Merge" : "Rebase" }}</h2><button class="x" @click="open = false">✕</button></div>
        <div class="body">
          <p class="intro">
            {{ mode === "merge"
              ? `Integrate another branch into ${currentBranch ?? "the current branch"}.`
              : `Rebase ${currentBranch ?? "the current branch"} onto another branch.` }}
          </p>
          <label class="field">
            <span>{{ mode === "merge" ? "Merge branch" : "Rebase onto" }}</span>
            <select v-model="branch">
              <option v-for="b in branches" :key="b" :value="b" :disabled="b === currentBranch">{{ b }}</option>
            </select>
          </label>

          <template v-if="mode === 'merge'">
            <label class="check"><input type="checkbox" v-model="squash" /> Squash commits <em>— apply changes as one commit</em></label>
            <label class="check"><input type="checkbox" v-model="noFf" /> Always create a merge commit <em>(--no-ff)</em></label>
            <label class="check"><input type="checkbox" v-model="noCommit" /> Don't commit — stage only <em>(--no-commit)</em></label>
            <label class="check"><input type="checkbox" v-model="verifySignatures" /> Verify signatures <em>— abort if the tip isn't validly signed</em></label>
          </template>
          <template v-else>
            <label class="check"><input type="checkbox" v-model="autostash" /> Auto-stash local changes <em>— stash & reapply around the rebase</em></label>
          </template>
          <label class="check"><input type="checkbox" v-model="noVerify" /> Skip hooks <em>(--no-verify)</em></label>

          <div class="actions">
            <button class="btn-accent" @click="run">{{ mode === "merge" ? "Merge" : "Rebase" }}</button>
            <button class="btn" @click="open = false">Cancel</button>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 520px; max-width: calc(100vw - 48px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); }
.head { display: flex; align-items: center; padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); }
.intro { font-size: 12.5px; color: var(--text-mid); margin: 0 0 var(--space-4); }
.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--space-3); font-size: 11px; color: var(--text-dim); }
.field select { height: 34px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field select:focus { outline: none; border-color: var(--accent); }
.check { display: flex; align-items: baseline; gap: 8px; font-size: 12.5px; color: var(--text); margin-bottom: var(--space-2); cursor: pointer; }
.check em { color: var(--text-faint); font-style: normal; font-size: 11px; }
.actions { display: flex; gap: var(--space-2); margin-top: var(--space-4); }
.btn-accent { height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn { height: 34px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
</style>
