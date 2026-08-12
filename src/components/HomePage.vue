<script setup lang="ts">
// First-launch / home screen — design plate 11. Hero on the left, recently
// opened + the "numbers, not adjectives" stat block on the right.
import { relativeTime } from "../lib/format";
import type { RecentRepo } from "../lib/recents";
import PlumbMark from "./PlumbMark.vue";

defineProps<{ recents: RecentRepo[] }>();
defineEmits<{
  (e: "open"): void;
  (e: "clone"): void;
  (e: "connect"): void;
  (e: "select", path: string): void;
  (e: "forget", path: string): void;
}>();

const abbr = (p: string) => p.replace(/^\/Users\/[^/]+/, "~");
</script>

<template>
  <main class="home">
    <div class="hero">
      <PlumbMark :size="76" class="hero-mark" />
      <h1>A straight line through your history.</h1>
      <p class="lead">
        Open a repository to start. No account, no trial, no upsell — Plumb is a native Mac app and
        it stays free.
      </p>
      <div class="cta">
        <button class="btn accent" @click="$emit('open')">Open a repository <kbd>⌘O</kbd></button>
        <button class="btn" @click="$emit('clone')">Clone from a URL <kbd>⌘⇧O</kbd></button>
        <button class="btn" @click="$emit('connect')">Connect an account <kbd>⌘N</kbd></button>
      </div>
      <p class="fine">
        Connecting GitHub or GitLab is optional — it adds pull/merge requests, nothing else.
      </p>
    </div>

    <aside class="side">
      <div class="side-label">Recently opened</div>
      <div class="recents">
        <div v-for="r in recents" :key="r.path" class="recent" @click="$emit('select', r.path)">
          <div class="r-main">
            <div class="r-name">{{ r.name }}</div>
            <div class="r-sub mono">{{ abbr(r.path) }} · {{ r.branch || "—" }} · {{ relativeTime(Math.floor(r.at / 1000)) }}</div>
          </div>
          <button class="r-x" title="Remove" @click.stop="$emit('forget', r.path)">✕</button>
        </div>
        <div v-if="!recents.length" class="recents-empty">Nothing yet — open a repository to begin.</div>
      </div>
    </aside>
  </main>
</template>

<style scoped>
.home { flex: 1; display: grid; grid-template-columns: 1fr 460px; min-height: 0; }

.hero {
  padding: 72px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  border-right: 2px solid var(--line);
  min-width: 0;
  color: var(--text);
}
.hero-mark { margin-bottom: 36px; }
.hero h1 {
  font-weight: 800;
  font-size: 52px;
  line-height: 1.02;
  letter-spacing: -0.03em;
  margin: 0 0 18px;
  max-width: 16ch;
}
.lead { font-size: 16px; line-height: 1.5; color: var(--text-mid); max-width: 48ch; margin: 0 0 44px; }
.cta { display: flex; gap: 2px; margin-bottom: 20px; flex-wrap: wrap; }
.btn {
  display: inline-flex; align-items: center; gap: var(--space-2);
  font-size: 13px; font-weight: 600; padding: 12px 20px;
  background: transparent; border: 1px solid var(--text-faint); color: var(--text);
  cursor: pointer;
}
.btn.accent { background: var(--accent); color: var(--accent-on); border-color: var(--accent); font-weight: 700; padding: 12px 22px; }
.btn kbd { font-family: var(--font-mono); font-size: 11px; color: var(--text-dim); }
.btn.accent kbd { color: var(--accent-on); opacity: 0.7; }
.fine { font-size: 12.5px; color: var(--text-faint); margin: 0; }

.side { padding: 44px 40px; display: flex; flex-direction: column; background: var(--subtle); min-height: 0; }
.side-label { font-size: 11px; font-weight: 600; letter-spacing: 0.12em; text-transform: uppercase; color: var(--text-faint); margin-bottom: 16px; }
.recents { overflow-y: auto; }
.recent {
  height: 52px; display: flex; align-items: center; gap: var(--space-2);
  border-bottom: 1px solid var(--raised); cursor: pointer;
}
.recent:hover { background: color-mix(in srgb, var(--raised) 60%, transparent); }
.recent:hover .r-x { opacity: 1; }
.r-main { flex: 1; min-width: 0; }
.r-name { font-size: 13px; font-weight: 600; }
.r-sub { font-size: 10.5px; color: var(--text-faint); margin-top: 3px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.r-x { flex: none; width: 22px; height: 22px; background: transparent; border: none; color: var(--text-faint); cursor: pointer; opacity: 0; }
.recents-empty { font-size: 12.5px; color: var(--text-faint); padding: var(--space-3) 0; }
</style>
