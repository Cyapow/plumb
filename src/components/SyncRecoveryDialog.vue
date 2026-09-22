<script setup lang="ts">
// Shown when a push is rejected (the upstream has commits we don't) or a pull
// is refused (uncommitted changes would be overwritten / branches diverged).
// It only collects the user's choice; App.vue runs the actual sequence so the
// normal syncing state, refresh and conflict handling apply.
import { computed } from "vue";

export type RecoveryChoice = "rebase" | "merge" | "force";

const open = defineModel<boolean>({ required: true });
const props = defineProps<{
  /** Which action failed. */
  reason: "push" | "pull";
  /** e.g. "origin/main". */
  upstream: string | null;
  /** Commits on the upstream that aren't local, and vice versa. */
  behind: number;
  ahead: number;
  /** Uncommitted working-tree changes that will be stashed around the pull. */
  dirty: number;
  /** Git's own message, for the curious. */
  error: string;
}>();
const emit = defineEmits<{ (e: "choose", choice: RecoveryChoice): void }>();

const n = (k: number, one: string, many = `${one}s`) => `${k} ${k === 1 ? one : many}`;
const title = computed(() => (props.reason === "push" ? "Push rejected" : "Pull blocked"));
const summary = computed(() => {
  const up = props.upstream ?? "the upstream";
  if (props.reason === "push") {
    return props.behind > 0
      ? `${up} has ${n(props.behind, "commit")} you don't have yet. Bring them in first, then push.`
      : `${up} has moved since you last fetched. Bring its changes in first, then push.`;
  }
  return props.dirty > 0
    ? `You have ${n(props.dirty, "uncommitted change")} that a pull would overwrite.`
    : `Your branch and ${up} have diverged; choose how to combine them.`;
});
const stashNote = computed(() => (props.dirty > 0 ? `Your ${n(props.dirty, "uncommitted change")} are stashed first and reapplied after.` : ""));
const then = computed(() => (props.reason === "push" ? ", then push" : ""));

function choose(c: RecoveryChoice) {
  open.value = false;
  emit("choose", c);
}
</script>

<template>
  <teleport to="body">
    <div v-if="open" class="backdrop" @click.self="open = false">
      <div class="sheet">
        <div class="head"><h2>{{ title }}</h2><button class="x" @click="open = false">✕</button></div>
        <div class="body">
          <p class="intro">{{ summary }}</p>
          <p v-if="stashNote" class="note">{{ stashNote }}</p>

          <button class="opt" @click="choose('rebase')">
            <div class="opt-t">Rebase onto {{ upstream ?? "upstream" }}{{ then }} <span class="rec">Recommended</span></div>
            <div class="opt-s">Replays your {{ ahead > 0 ? n(ahead, "commit") : "commits" }} on top of theirs — keeps history linear, no merge commit.</div>
          </button>
          <button class="opt" @click="choose('merge')">
            <div class="opt-t">Merge {{ upstream ?? "upstream" }} in{{ then }}</div>
            <div class="opt-s">Creates a merge commit joining both lines of work.</div>
          </button>
          <button v-if="reason === 'push'" class="opt danger" @click="choose('force')">
            <div class="opt-t">Force push (with lease)</div>
            <div class="opt-s">Overwrite {{ upstream ?? "the remote" }} with your branch. Their {{ behind > 0 ? n(behind, "commit") : "commits" }} will be dropped. Refuses if the remote moved again since your last fetch.</div>
          </button>

          <details class="raw">
            <summary>What Git said</summary>
            <pre>{{ error }}</pre>
          </details>

          <div class="actions">
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
.intro { font-size: 13px; color: var(--text); margin: 0 0 var(--space-2); line-height: 1.5; }
.note { font-size: 12px; color: var(--text-mid); margin: 0 0 var(--space-4); line-height: 1.5; }
.opt { display: block; width: 100%; text-align: left; padding: var(--space-3); margin-bottom: var(--space-2); background: var(--raised); border: 1px solid var(--line); color: var(--text); cursor: pointer; }
.opt:hover { border-color: var(--accent); }
.opt-t { font-size: 13px; font-weight: 700; display: flex; align-items: center; gap: var(--space-2); }
.opt-s { font-size: 11.5px; color: var(--text-mid); margin-top: 3px; line-height: 1.45; }
.rec { font-size: 10px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; padding: 1px 6px; background: var(--accent); color: var(--accent-on); }
.opt.danger:hover { border-color: #e5484d; }
.opt.danger .opt-t { color: #e5484d; }
.raw { margin-top: var(--space-3); font-size: 11.5px; color: var(--text-faint); }
.raw summary { cursor: pointer; }
.raw pre { margin: var(--space-2) 0 0; padding: var(--space-2); font-family: var(--font-mono); font-size: 11px; white-space: pre-wrap; word-break: break-word; background: var(--surface); border: 1px solid var(--line-soft); max-height: 160px; overflow: auto; user-select: text; }
.actions { display: flex; gap: var(--space-2); margin-top: var(--space-4); justify-content: flex-end; }
.btn { height: 34px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
</style>
