<script setup lang="ts">
import { ref, watch } from "vue";
import { stashSaveEx } from "../lib/git";
import { toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string }>();
const emit = defineEmits<{ (e: "done"): void }>();

const message = ref("");
const includeUntracked = ref(true);
const keepIndex = ref(false);
const busy = ref(false);

watch(open, (o) => {
  if (!o) return;
  message.value = "";
  includeUntracked.value = true;
  keepIndex.value = false;
});

async function save() {
  busy.value = true;
  try {
    await stashSaveEx(props.repoPath, message.value.trim() || null, includeUntracked.value, keepIndex.value);
    toast("Changes stashed");
    open.value = false;
    emit("done");
  } catch (e) {
    toast("Stash failed", String(e), "error");
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head"><h2>Save stash</h2><button class="x" @click="open = false">✕</button></div>
        <div class="body">
          <label class="field"><span>Message (optional)</span><input v-model="message" @keydown.enter="save" spellcheck="false" autofocus /></label>
          <label class="check"><input type="checkbox" v-model="includeUntracked" /> Include untracked files</label>
          <label class="check"><input type="checkbox" v-model="keepIndex" /> Keep staged changes in the working copy</label>
          <div class="actions">
            <button class="btn-accent" :disabled="busy" @click="save">Save stash</button>
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
.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--space-3); font-size: 11px; color: var(--text-dim); }
.field input { height: 34px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field input:focus { outline: none; border-color: var(--accent); }
.check { display: flex; align-items: center; gap: 8px; font-size: 12.5px; color: var(--text); margin-bottom: var(--space-2); cursor: pointer; }
.actions { display: flex; gap: var(--space-2); margin-top: var(--space-4); }
.btn-accent { height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.6; }
.btn { height: 34px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
</style>
