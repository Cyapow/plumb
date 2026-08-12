<script setup lang="ts">
// List submodules with sync state, update them, or open one as its own repo.
import { ref, watch } from "vue";
import { listSubmodules, updateSubmodules, type SubmoduleInfo } from "../lib/git";
import { toast } from "../lib/ui";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{ repoPath: string }>();
const emit = defineEmits<{ (e: "open", path: string): void }>();

const subs = ref<SubmoduleInfo[]>([]);
const busy = ref(false);

async function reload() {
  subs.value = await listSubmodules(props.repoPath).catch(() => []);
}
watch(open, (o) => o && reload());

async function updateAll() {
  busy.value = true;
  try {
    const msg = await updateSubmodules(props.repoPath, true);
    toast("Submodules", msg);
    await reload();
  } catch (e) {
    toast("Update failed", String(e), "error");
  } finally {
    busy.value = false;
  }
}

function state(s: SubmoduleInfo): { label: string; cls: string } {
  if (!s.initialized) return { label: "not initialized", cls: "warn" };
  if (s.pinned_id && s.wd_id && s.pinned_id !== s.wd_id) return { label: "out of sync", cls: "warn" };
  if (s.modified) return { label: "modified", cls: "mod" };
  return { label: "up to date", cls: "ok" };
}

function openSub(s: SubmoduleInfo) {
  emit("open", `${props.repoPath.replace(/\/$/, "")}/${s.path}`);
  open.value = false;
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head">
          <h2>Submodules</h2>
          <span class="grow"></span>
          <button class="btn-accent" :disabled="busy || !subs.length" @click="updateAll">
            {{ busy ? "Updating…" : "Update all" }}
          </button>
          <button class="x" @click="open = false">✕</button>
        </div>
        <div class="body">
          <div v-if="!subs.length" class="empty">This repository has no submodules.</div>
          <div v-for="s in subs" :key="s.name" class="sub">
            <div class="s-top">
              <span class="s-name mono">{{ s.path }}</span>
              <span class="badge" :class="state(s).cls">{{ state(s).label }}</span>
              <span class="grow"></span>
              <button class="mini" :disabled="!s.initialized" @click="openSub(s)">Open</button>
            </div>
            <div class="s-url mono">{{ s.url }}</div>
          </div>
        </div>
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 1200; background: color-mix(in srgb, #000 55%, transparent); display: flex; align-items: center; justify-content: center; }
.sheet { width: 620px; max-width: calc(100vw - 48px); max-height: 74vh; background: var(--surface); border: 1px solid var(--line); box-shadow: var(--shadow-lg); display: flex; flex-direction: column; }
.head { display: flex; align-items: center; gap: var(--space-2); padding: var(--space-4); border-bottom: 2px solid var(--line); }
.head h2 { margin: 0; font-size: 17px; font-weight: 800; }
.head .grow { flex: 1; }
.head .btn-accent { height: 30px; padding: 0 14px; background: var(--accent); color: var(--accent-on); border: none; font-weight: 700; font-size: 12px; cursor: pointer; }
.head .btn-accent:disabled { opacity: 0.5; }
.head .x { width: 30px; height: 26px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.body { padding: var(--space-4); overflow-y: auto; }
.empty { font-size: 12.5px; color: var(--text-dim); }
.sub { border: 1px solid var(--line); padding: 10px 12px; margin-bottom: var(--space-2); }
.s-top { display: flex; align-items: center; gap: var(--space-2); }
.s-name { font-size: 13px; font-weight: 700; }
.badge { font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; padding: 2px 7px; color: var(--accent-on); }
.badge.ok { background: var(--lane-3); }
.badge.mod { background: var(--lane-2); }
.badge.warn { background: var(--accent); }
.grow { flex: 1; }
.mini { padding: 3px 10px; font-size: 11px; background: var(--raised); border: 1px solid var(--line); color: var(--text-mid); cursor: pointer; }
.mini:disabled { opacity: 0.5; }
.s-url { font-size: 11px; color: var(--text-dim); margin-top: 6px; word-break: break-all; }
</style>
