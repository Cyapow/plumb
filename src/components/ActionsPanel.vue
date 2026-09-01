<script setup lang="ts">
// Editor for Custom Actions — user-defined commands surfaced in the toolbar and
// context menus. Each is a program plus argument lines; placeholders expand at
// run time. Saved to the backend config.
import { onMounted, ref } from "vue";
import {
  actionsStore,
  refreshActions,
  saveActions,
  blankAction,
  type CustomAction,
  type ActionContext,
} from "../lib/actions";
import { toast } from "../lib/ui";

const draft = ref<CustomAction[]>([]);
const saving = ref(false);

onMounted(async () => {
  await refreshActions();
  draft.value = actionsStore.list.map((a) => ({ ...a, args: [...a.args] }));
});

const contexts: { id: ActionContext; label: string }[] = [
  { id: "toolbar", label: "Toolbar" },
  { id: "commit", label: "Commit menu" },
  { id: "branch", label: "Branch menu" },
  { id: "file", label: "File menu" },
];

function add() {
  draft.value.push(blankAction());
}
function remove(i: number) {
  draft.value.splice(i, 1);
}
// Args are edited as one-per-line text; keep the array in sync.
function argsText(a: CustomAction): string {
  return a.args.join("\n");
}
function setArgs(a: CustomAction, text: string) {
  a.args = text.split("\n").map((s) => s.replace(/\r$/, ""));
}

async function save() {
  // Drop rows with no label or program.
  const clean = draft.value
    .map((a) => ({ ...a, label: a.label.trim(), program: a.program.trim(), args: a.args.filter((s) => s.length > 0) }))
    .filter((a) => a.label && a.program);
  saving.value = true;
  try {
    await saveActions(clean);
    await refreshActions();
    draft.value = actionsStore.list.map((a) => ({ ...a, args: [...a.args] }));
    toast("Actions saved", `${clean.length} action${clean.length === 1 ? "" : "s"}`);
  } catch (e) {
    toast("Couldn't save actions", String(e), "error");
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="actions-panel">
    <div class="head">
      <div>
        <div class="row-title">Custom Actions</div>
        <div class="row-sub">
          Run your own commands from the toolbar and context menus. The program runs in the repo folder;
          arguments are passed literally (no shell). Placeholders:
          <span class="mono">{repo} {sha} {shortSha} {branch} {file}</span>.
        </div>
      </div>
      <span class="grow"></span>
      <button class="btn" @click="add">Add action</button>
      <button class="btn accent" :disabled="saving" @click="save">{{ saving ? "Saving…" : "Save" }}</button>
    </div>

    <div v-if="!draft.length" class="empty">
      No actions yet. Add one — e.g. open the repo in your editor:
      program <span class="mono">code</span>, argument <span class="mono">{repo}</span>.
    </div>

    <div v-for="(a, i) in draft" :key="i" class="card">
      <div class="line">
        <label class="fld grow"><span>Label</span><input v-model="a.label" placeholder="Open in VS Code" spellcheck="false" /></label>
        <label class="fld"><span>Shows in</span>
          <select v-model="a.context">
            <option v-for="c in contexts" :key="c.id" :value="c.id">{{ c.label }}</option>
          </select>
        </label>
        <button class="del" title="Remove" @click="remove(i)">✕</button>
      </div>
      <div class="line">
        <label class="fld prog"><span>Program</span><input v-model="a.program" placeholder="code" spellcheck="false" /></label>
        <label class="chk"><input type="checkbox" v-model="a.confirm" /> Confirm first</label>
      </div>
      <label class="fld"><span>Arguments — one per line</span>
        <textarea :value="argsText(a)" rows="2" spellcheck="false" placeholder="{repo}" @input="setArgs(a, ($event.target as HTMLTextAreaElement).value)"></textarea>
      </label>
    </div>
  </div>
</template>

<style scoped>
.actions-panel { display: flex; flex-direction: column; gap: var(--space-4); }
.head { display: flex; align-items: flex-start; gap: var(--space-3); }
.head .grow { flex: 1; }
.row-title { font-size: 13px; font-weight: 600; }
.row-sub { font-size: 11.5px; color: var(--text-faint); margin-top: 2px; max-width: 640px; line-height: 1.5; }
.btn { height: 30px; padding: 0 14px; background: var(--raised); border: 1px solid var(--line); color: var(--text); font-size: 12px; cursor: pointer; }
.btn.accent { background: var(--accent); color: var(--accent-on); border-color: var(--accent); font-weight: 700; }
.btn:disabled { opacity: 0.5; }
.empty { font-size: 12.5px; color: var(--text-dim); padding: var(--space-4); border: 1px dashed var(--line); }
.card { border: 1px solid var(--line); padding: var(--space-3); display: flex; flex-direction: column; gap: var(--space-3); background: var(--subtle); }
.line { display: flex; align-items: flex-end; gap: var(--space-3); }
.fld { display: flex; flex-direction: column; gap: 4px; font-size: 11px; color: var(--text-dim); }
.fld.grow { flex: 1; }
.fld.prog { width: 240px; }
.fld input, .fld select, .fld textarea { background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; padding: 6px 10px; }
.fld textarea { font-family: var(--font-mono); resize: vertical; }
.fld input:focus, .fld select:focus, .fld textarea:focus { outline: none; border-color: var(--accent); }
.chk { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--text); white-space: nowrap; padding-bottom: 6px; }
.del { width: 30px; height: 32px; flex: none; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); cursor: pointer; }
.del:hover { color: var(--accent); border-color: var(--accent); }
</style>
