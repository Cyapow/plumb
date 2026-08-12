<script setup lang="ts">
// Start a bisect: mark a known-bad and known-good commit; git then walks the
// midpoints for you (drive them from the banner).
import { ref, watch } from "vue";
import { bisectStart } from "../lib/git";
import { toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string; branches: string[] }>();
const emit = defineEmits<{ (e: "started", message: string): void }>();

const bad = ref("HEAD");
const good = ref("");
const busy = ref(false);
const error = ref<string | null>(null);

watch(open, (o) => {
  if (!o) return;
  bad.value = "HEAD";
  good.value = "";
  error.value = null;
});

async function start() {
  if (!good.value.trim()) {
    error.value = "Enter a commit you know was good.";
    return;
  }
  busy.value = true;
  error.value = null;
  try {
    const msg = await bisectStart(props.repoPath, bad.value.trim() || "HEAD", good.value.trim());
    toast("Bisect started", "Test the checked-out commit, then mark it.");
    open.value = false;
    emit("started", msg);
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
        <div class="head"><h2>Start bisect</h2><button class="x" @click="open = false">✕</button></div>
        <div class="body">
          <p class="intro">
            Bisect finds the commit that introduced a bug by binary search. Give it a commit where the bug
            <b>exists</b> and one where it <b>didn't</b>; git checks out the midpoint for you to test.
          </p>
          <label class="field">
            <span>Bad (bug present)</span>
            <input v-model="bad" list="bi-branches" spellcheck="false" placeholder="HEAD" />
          </label>
          <label class="field">
            <span>Good (bug absent)</span>
            <input v-model="good" list="bi-branches" spellcheck="false" placeholder="a commit / tag / branch" />
          </label>
          <datalist id="bi-branches"><option v-for="b in branches" :key="b" :value="b" /></datalist>
          <p v-if="error" class="err mono">{{ error }}</p>
          <div class="actions">
            <button class="btn-accent" :disabled="busy" @click="start">{{ busy ? "Starting…" : "Start bisect" }}</button>
            <button class="btn" @click="open = false">Cancel</button>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 540px; max-width: calc(100vw - 48px); background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); }
.head { display: flex; align-items: center; padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .x { margin-left: auto; width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); }
.intro { font-size: 12.5px; color: var(--text-mid); line-height: 1.6; margin: 0 0 var(--space-4); }
.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--space-3); font-size: 11px; color: var(--text-dim); }
.field input { height: 34px; padding: 0 10px; background: var(--bg); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field input:focus { outline: none; border-color: var(--accent); }
.err { color: var(--accent); font-size: 11px; margin: 0 0 var(--space-3); }
.actions { display: flex; gap: var(--space-2); }
.btn-accent { height: 34px; padding: 0 18px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.6; }
.btn { height: 34px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
</style>
