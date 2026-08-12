<script setup lang="ts">
// Resolve merge/rebase/cherry-pick conflicts: pick a whole side or hand-edit
// the result, file by file, then continue the operation.
import { computed, ref, watch } from "vue";
import {
  listConflicts,
  conflictSides,
  resolveConflict,
  resolveConflictContent,
  opContinue,
  type ConflictSides,
} from "../lib/git";
import { toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string }>();
const emit = defineEmits<{ (e: "resolved"): void }>();

const files = ref<string[]>([]);
const active = ref<string | null>(null);
const sides = ref<ConflictSides | null>(null);
const result = ref("");
const busy = ref(false);

const allResolved = computed(() => files.value.length === 0);

async function reload(keep?: string) {
  files.value = await listConflicts(props.repoPath).catch(() => []);
  if (files.value.length === 0) {
    active.value = null;
    sides.value = null;
    return;
  }
  const next = keep && files.value.includes(keep) ? keep : files.value[0];
  await select(next);
}

async function select(file: string) {
  active.value = file;
  sides.value = await conflictSides(props.repoPath, file);
  result.value = sides.value.merged;
}

watch(open, (o) => {
  if (o) reload();
});

async function take(side: "ours" | "theirs") {
  if (!active.value) return;
  busy.value = true;
  try {
    await resolveConflict(props.repoPath, active.value, side);
    await afterResolve();
  } catch (e) {
    toast("Resolve failed", String(e), "error");
  } finally {
    busy.value = false;
  }
}

async function markResolved() {
  if (!active.value) return;
  busy.value = true;
  try {
    await resolveConflictContent(props.repoPath, active.value, result.value);
    await afterResolve();
  } catch (e) {
    toast("Resolve failed", String(e), "error");
  } finally {
    busy.value = false;
  }
}

async function afterResolve() {
  toast("File resolved", active.value ?? "");
  await reload();
  emit("resolved");
}

async function finish() {
  busy.value = true;
  try {
    const msg = await opContinue(props.repoPath);
    toast("Continued", msg);
    open.value = false;
    emit("resolved");
  } catch (e) {
    toast("Couldn't continue", String(e), "error");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop">
      <div class="sheet">
        <div class="head">
          <h2>Resolve conflicts</h2>
          <span class="sub mono">{{ files.length }} file{{ files.length === 1 ? "" : "s" }} left</span>
          <button class="x" @click="open = false">✕</button>
        </div>

        <div v-if="allResolved" class="done">
          <p>All conflicts resolved.</p>
          <button class="btn-accent" :disabled="busy" @click="finish">Continue operation</button>
        </div>

        <div v-else class="grid">
          <aside class="files">
            <div
              v-for="f in files"
              :key="f"
              class="file"
              :class="{ on: f === active }"
              @click="select(f)"
            >
              <span class="u">U</span><span class="fname mono">{{ f }}</span>
            </div>
          </aside>

          <section v-if="sides" class="editor">
            <div class="sides">
              <div class="side">
                <div class="side-head">
                  <span>Ours (current)</span>
                  <button :disabled="busy" @click="take('ours')">Use ours</button>
                </div>
                <pre class="mono code">{{ sides.ours ?? "— side removed —" }}</pre>
              </div>
              <div class="side">
                <div class="side-head">
                  <span>Theirs (incoming)</span>
                  <button :disabled="busy" @click="take('theirs')">Use theirs</button>
                </div>
                <pre class="mono code">{{ sides.theirs ?? "— side removed —" }}</pre>
              </div>
            </div>

            <div class="result">
              <div class="result-head">
                <span>Result — edit, remove the &lt;&lt;&lt;&lt; markers, then mark resolved</span>
                <button class="btn-accent" :disabled="busy" @click="markResolved">Mark resolved</button>
              </div>
              <textarea v-model="result" class="mono" spellcheck="false"></textarea>
            </div>
          </section>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 62%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 1040px; max-width: calc(100vw - 40px); height: 82vh; background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; }
.head { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 16px; font-weight: 800; }
.sub { font-size: 11px; color: var(--text-dim); }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.done { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--space-4); color: var(--text-mid); }
.grid { flex: 1; display: grid; grid-template-columns: 240px 1fr; min-height: 0; }
.files { border-right: 1px solid var(--line); overflow-y: auto; }
.file { display: flex; align-items: center; gap: 8px; padding: 8px 12px; cursor: pointer; font-size: 12px; }
.file:hover { background: var(--raised); }
.file.on { background: color-mix(in srgb, var(--accent) 16%, var(--surface)); }
.file .u { flex: none; width: 16px; height: 16px; display: grid; place-items: center; font-size: 9px; font-weight: 800; color: var(--accent-on); background: var(--accent); }
.fname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left; }
.editor { display: flex; flex-direction: column; min-height: 0; }
.sides { display: grid; grid-template-columns: 1fr 1fr; gap: 1px; background: var(--line); flex: 1; min-height: 0; }
.side { display: flex; flex-direction: column; background: var(--surface); min-height: 0; }
.side-head { display: flex; align-items: center; padding: 6px 10px; font-size: 11px; color: var(--text-dim); border-bottom: 1px solid var(--line); }
.side-head button { margin-left: auto; padding: 3px 9px; font-size: 11px; background: var(--raised); border: 1px solid var(--line); color: var(--text); cursor: pointer; }
.side-head button:disabled { opacity: 0.5; }
.code { flex: 1; margin: 0; padding: 10px; overflow: auto; font-size: 11.5px; line-height: 1.5; white-space: pre; color: var(--text); }
.result { display: flex; flex-direction: column; height: 40%; border-top: 2px solid var(--line); }
.result-head { display: flex; align-items: center; padding: 6px 10px; font-size: 11px; color: var(--text-dim); }
.result-head .btn-accent { margin-left: auto; padding: 4px 12px; font-size: 11.5px; font-weight: 700; background: var(--accent); color: var(--accent-on); border: none; cursor: pointer; }
.result-head .btn-accent:disabled { opacity: 0.5; }
.result textarea { flex: 1; resize: none; border: none; background: var(--bg); color: var(--text); padding: 10px; font-size: 12px; line-height: 1.5; outline: none; }
.btn-accent { padding: 8px 18px; font-weight: 700; background: var(--accent); color: var(--accent-on); border: none; cursor: pointer; }
</style>
