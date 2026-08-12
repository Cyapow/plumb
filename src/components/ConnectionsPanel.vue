<script setup lang="ts">
// Manage GitHub / GitLab accounts (multiple each, incl. self-managed).
// Tokens are validated on connect and stored in the macOS Keychain.
import { computed, onMounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  connectAccount,
  removeConnection,
  testConnection,
  githubDeviceStart,
  githubDevicePoll,
  gitlabOauthLogin,
  type Connection,
  type DeviceCode,
} from "../lib/accounts";
import { connectionsStore, refreshConnections, toast } from "../lib/ui";

const cfg = computed(() => connectionsStore.config);
const adding = ref(false);
const testing = ref<string | null>(null);

type Provider = "github" | "gitlab";
const provider = ref<Provider>("github");
const baseUrl = ref("");
const token = ref("");
const label = ref("");
const busy = ref(false);
const formError = ref<string | null>(null);

const DEFAULTS: Record<
  Provider,
  { name: string; base: string; scopes: string; tokenPath: (host: string) => string }
> = {
  github: {
    name: "GitHub",
    base: "https://api.github.com",
    scopes: "repo, read:user",
    tokenPath: () => "https://github.com/settings/tokens/new?scopes=repo,read:user&description=Plumb",
  },
  gitlab: {
    name: "GitLab",
    base: "https://gitlab.com",
    scopes: "api",
    tokenPath: (host) =>
      `${host.replace(/\/$/, "")}/-/user_settings/personal_access_tokens?name=Plumb&scopes=api`,
  },
};

onMounted(() => {
  refreshConnections();
  if (!cfg.value.connections.length) startAdd();
});

function startAdd() {
  adding.value = true;
  provider.value = "github";
  applyProvider();
  token.value = "";
  label.value = "";
  formError.value = null;
}
function applyProvider() {
  baseUrl.value = DEFAULTS[provider.value].base;
}

function getToken() {
  const host =
    provider.value === "gitlab"
      ? baseUrl.value.replace("https://", "https://").replace(/\/?$/, "")
      : "";
  // For GitLab self-managed, the token page lives on the instance host, not the API base.
  const instance = provider.value === "gitlab" ? (host || "https://gitlab.com") : "";
  openUrl(DEFAULTS[provider.value].tokenPath(instance));
}

async function connect() {
  formError.value = null;
  if (!token.value.trim()) return (formError.value = "Paste a personal access token.");
  if (!baseUrl.value.trim()) return (formError.value = "Enter the API base URL.");
  busy.value = true;
  try {
    await connectAccount(provider.value, baseUrl.value.trim(), token.value.trim(), label.value.trim() || undefined);
    await refreshConnections();
    adding.value = false;
    toast("Account connected");
  } catch (e) {
    formError.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function test(c: Connection) {
  testing.value = c.id;
  try {
    toast("Connection OK", await testConnection(c.id));
  } catch (e) {
    toast("Connection failed", String(e), "error");
  } finally {
    testing.value = null;
  }
}
async function remove(c: Connection) {
  if (!window.confirm(`Disconnect ${c.label}? Its token is removed from the Keychain.`)) return;
  await removeConnection(c.id);
  await refreshConnections();
}

const host = (c: Connection) => c.baseUrl.replace(/^https?:\/\//, "").replace(/\/$/, "");

/* ── OAuth one-click (client_id kept in localStorage) ─────────────── */
const LS: Record<Provider, string> = {
  github: "plumb.oauth.github.clientId",
  gitlab: "plumb.oauth.gitlab.clientId",
};
const REGISTER: Record<Provider, string> = {
  github: "https://github.com/settings/applications/new",
  gitlab: "https://gitlab.com/-/user_settings/applications",
};
const clientIds = ref<Record<Provider, string>>({
  github: localStorage.getItem(LS.github) ?? "",
  gitlab: localStorage.getItem(LS.gitlab) ?? "",
});
const oauthSetup = ref<Provider | null>(null);
const clientIdInput = ref("");
const oauthBusy = ref<Provider | null>(null);
const device = ref<DeviceCode | null>(null);

function beginOAuth(p: Provider) {
  if (!clientIds.value[p]) {
    oauthSetup.value = p;
    clientIdInput.value = "";
    return;
  }
  runOAuth(p);
}
function saveClientId() {
  const p = oauthSetup.value;
  if (!p) return;
  const id = clientIdInput.value.trim();
  if (!id) return;
  clientIds.value[p] = id;
  localStorage.setItem(LS[p], id);
  oauthSetup.value = null;
  runOAuth(p);
}
async function runOAuth(p: Provider) {
  oauthBusy.value = p;
  try {
    let conn: Connection;
    if (p === "github") {
      const dc = await githubDeviceStart(clientIds.value.github);
      device.value = dc;
      // Copy the code so connecting is just: switch to GitHub, paste, authorize.
      copyCode(dc.userCode);
      openUrl(dc.verificationUri);
      conn = await githubDevicePoll(clientIds.value.github, dc.deviceCode, dc.interval);
      device.value = null;
    } else {
      conn = await gitlabOauthLogin(clientIds.value.gitlab);
    }
    await refreshConnections();
    adding.value = false;
    toast("Account connected", conn.username);
  } catch (e) {
    device.value = null;
    toast("OAuth failed", String(e), "error");
  } finally {
    oauthBusy.value = null;
  }
}

async function copyCode(code: string) {
  try {
    await navigator.clipboard.writeText(code);
    toast("Code copied", "Paste it in the GitHub tab");
  } catch {
    /* clipboard blocked — the code is still shown to type manually */
  }
}

function cancelDevice() {
  device.value = null;
  oauthBusy.value = null;
}
</script>

<template>
  <div class="panel">
    <div class="list" v-if="cfg.connections.length">
      <div v-for="c in cfg.connections" :key="c.id" class="conn">
        <img v-if="c.avatarUrl" :src="c.avatarUrl" class="avatar" alt="" />
        <span class="badge" :class="c.provider">{{ c.provider === "github" ? "GH" : "GL" }}</span>
        <div class="meta">
          <div class="label">{{ c.label }}</div>
          <div class="sub mono">{{ c.username }} · {{ host(c) }}</div>
        </div>
        <div class="actions">
          <button class="mini" :disabled="testing === c.id" @click="test(c)">{{ testing === c.id ? "…" : "Test" }}</button>
          <button class="mini danger" @click="remove(c)">Disconnect</button>
        </div>
      </div>
    </div>

    <div v-if="adding" class="add">
      <!-- One-click OAuth -->
      <div class="oauth">
        <button class="oauth-btn" :disabled="!!oauthBusy" @click="beginOAuth('github')">
          <span v-if="oauthBusy === 'github'" class="spinner-sm"></span>Connect with GitHub
        </button>
        <button class="oauth-btn" :disabled="!!oauthBusy" @click="beginOAuth('gitlab')">
          <span v-if="oauthBusy === 'gitlab'" class="spinner-sm"></span>Connect with GitLab
        </button>
      </div>

      <div v-if="oauthSetup" class="oauth-setup">
        <div class="section-label">
          One-time setup · your {{ oauthSetup === "github" ? "GitHub" : "GitLab" }} OAuth app
          <a class="key-link" @click.prevent="openUrl(REGISTER[oauthSetup])">Register ↗</a>
        </div>
        <div class="model-row">
          <input v-model="clientIdInput" placeholder="paste the client ID" spellcheck="false" />
          <button class="mini" @click="saveClientId">Save &amp; connect</button>
        </div>
        <div v-if="oauthSetup === 'github'" class="scopes-hint mono">
          Enable "Device Flow" · scopes: repo, read:user
        </div>
        <div v-else class="scopes-hint mono">
          Redirect URI: http://127.0.0.1:47823/callback · scope: api · confidential: off
        </div>
      </div>

      <div v-if="device" class="device">
        <div class="device-head">
          <span>Enter this code at GitHub (copied to your clipboard):</span>
          <button class="x" title="Cancel" @click="cancelDevice">✕</button>
        </div>
        <div class="device-row">
          <span class="user-code mono">{{ device.userCode }}</span>
          <button class="mini" @click="copyCode(device.userCode)">Copy</button>
          <button class="mini" @click="openUrl(device.verificationUri)">Open GitHub ↗</button>
          <span class="waiting"><span class="spinner-sm"></span>Waiting…</span>
        </div>
      </div>

      <div class="or-sep"><span>or add a token manually</span></div>

      <div class="seg">
        <button :class="{ on: provider === 'github' }" @click="provider = 'github'; applyProvider()">GitHub</button>
        <button :class="{ on: provider === 'gitlab' }" @click="provider = 'gitlab'; applyProvider()">GitLab</button>
      </div>
      <label class="field">
        <span>API base URL <em>— change for Enterprise / self-managed</em></span>
        <input v-model="baseUrl" spellcheck="false" />
      </label>
      <label class="field">
        <span>Personal access token
          <a class="key-link" @click.prevent="getToken">Get a token ↗</a>
        </span>
        <input v-model="token" type="password" placeholder="paste token" spellcheck="false" autocomplete="off" />
        <div class="scopes-hint mono">Required scope{{ DEFAULTS[provider].scopes.includes(",") ? "s" : "" }}: {{ DEFAULTS[provider].scopes }}</div>
      </label>
      <label class="field">
        <span>Label</span>
        <input v-model="label" placeholder="optional (defaults to username)" spellcheck="false" />
      </label>
      <p v-if="formError" class="form-error mono">{{ formError }}</p>
      <div class="add-actions">
        <button class="btn-accent" :disabled="busy" @click="connect">{{ busy ? "Connecting…" : "Connect" }}</button>
        <button v-if="cfg.connections.length" class="btn" @click="adding = false">Cancel</button>
      </div>
    </div>

    <button v-else class="add-more" @click="startAdd">+ Connect another account</button>

    <div class="note">
      Multiple GitHub &amp; GitLab accounts are supported — including self-managed and Enterprise
      hosts. Tokens are validated on connect and stored in your macOS Keychain.
    </div>
  </div>
</template>

<style scoped>
.panel { font-size: 13px; }
.list { display: flex; flex-direction: column; gap: 2px; margin-bottom: var(--space-4); }
.conn { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-3); background: var(--raised); border: 1px solid var(--line); }
.avatar { width: 26px; height: 26px; flex: none; object-fit: cover; }
.badge { font-family: var(--font-mono); font-size: 9.5px; font-weight: 700; padding: 2px 5px; flex: none; border: 1px solid var(--text-dim); color: var(--text-mid); }
.badge.github { border-color: var(--text); color: var(--text); }
.badge.gitlab { border-color: var(--lane-2); color: var(--lane-2); }
.meta { min-width: 0; flex: 1; }
.label { font-size: 13px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sub { font-size: 10.5px; color: var(--text-faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.actions { display: flex; gap: 2px; flex: none; }
.mini { font-size: 11px; padding: 4px 8px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.mini.danger { color: var(--accent); }
.mini:disabled { opacity: 0.5; }

.add { border: 1px solid var(--line); padding: var(--space-4); background: var(--bg); }
.oauth { display: flex; gap: var(--space-2); }
.oauth-btn { flex: 1; display: flex; align-items: center; justify-content: center; gap: var(--space-2); padding: 10px; background: var(--accent); color: var(--accent-on); border: 1px solid var(--accent); font-weight: 700; font-size: 12px; cursor: pointer; }
.oauth-btn:disabled { opacity: 0.6; }
.spinner-sm { width: 11px; height: 11px; border: 2px solid color-mix(in srgb, var(--accent-on) 40%, transparent); border-top-color: var(--accent-on); border-radius: 50%; animation: plumb-spin 0.7s linear infinite; }
@keyframes plumb-spin { to { transform: rotate(360deg); } }
.oauth-setup { margin-top: var(--space-3); border: 1px solid var(--line); padding: var(--space-3); }
.oauth-setup .section-label { display: flex; align-items: center; margin-bottom: var(--space-2); }
.device { margin-top: var(--space-3); border: 1px solid var(--accent); padding: var(--space-3); font-size: 12px; }
.device-head { display: flex; align-items: center; }
.device-head .x { margin-left: auto; width: 22px; height: 20px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; }
.device-row { display: flex; align-items: center; gap: var(--space-3); margin-top: var(--space-2); }
.user-code { font-size: 20px; font-weight: 700; letter-spacing: 0.1em; color: var(--accent); }
.waiting { display: flex; align-items: center; gap: 6px; font-size: 11px; color: var(--text-faint); }
.or-sep { display: flex; align-items: center; text-align: center; color: var(--text-faint); font-size: 11px; margin: var(--space-4) 0; }
.or-sep::before, .or-sep::after { content: ""; flex: 1; height: 1px; background: var(--line); }
.or-sep span { padding: 0 var(--space-3); }
.seg { display: flex; gap: 2px; margin-bottom: var(--space-4); }
.seg button { flex: 1; padding: 7px 0; background: var(--raised); border: 1px solid var(--line); font-size: 12px; font-weight: 600; color: var(--text-mid); cursor: pointer; }
.seg button.on { background: var(--accent); color: var(--accent-on); border-color: var(--accent); }
.field { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--space-3); font-size: 11px; color: var(--text-dim); }
.field span { display: flex; align-items: center; }
.field em { color: var(--text-faint); font-style: normal; }
.key-link { margin-left: auto; color: var(--accent); cursor: pointer; }
.field input { height: 30px; padding: 0 10px; background: var(--surface); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field input:focus { outline: none; border-color: var(--accent); }
.scopes-hint { font-size: 10px; color: var(--text-faint); margin-top: 4px; }
.form-error { color: var(--accent); font-size: 11px; margin: 0 0 var(--space-3); }
.add-actions { display: flex; gap: var(--space-2); }
.btn-accent { height: 32px; padding: 0 16px; background: var(--accent); color: var(--accent-on); border: 1px solid var(--accent); font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.5; }
.btn { height: 32px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
.add-more { width: 100%; padding: var(--space-3); background: var(--raised); border: 1px dashed var(--line); cursor: pointer; font-size: 12.5px; color: var(--text-mid); }
.note { margin-top: var(--space-4); font-size: 11px; color: var(--text-faint); line-height: 1.5; border-top: 1px solid var(--line); padding-top: var(--space-3); }
</style>
