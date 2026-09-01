<script setup lang="ts">
// AI provider management. Low-resistance onboarding:
//  • OpenRouter one-click login (OAuth PKCE, no central server)
//  • auto-detect API keys already in your environment
//  • "Get a key" deep-links for direct OpenAI/Anthropic/Gemini
//  • model lists fetched from the provider API (no manual model guessing)
import { computed, onMounted, ref } from "vue";
import { openUrl } from "../lib/native";
import {
  saveAiProvider,
  saveAiProviderFromEnv,
  removeAiProvider,
  setDefaultAiProvider,
  testAiProvider,
  listProviderModels,
  detectEnvKeys,
  openrouterLogin,
  hasApiKey,
  type AiProvider,
  type EnvKey,
} from "../lib/ai";
import { aiStore, refreshAiConfig, toast, promptConfirm } from "../lib/ui";

const cfg = computed(() => aiStore.config);
const adding = ref(false);
const editId = ref<string | null>(null);
const keyIsSet = ref(false);
const testing = ref<string | null>(null);
const envKeys = ref<EnvKey[]>([]);
const orConnecting = ref(false);
const orPick = ref<{ providerId: string; models: string[]; model: string } | null>(null);

type Vendor = "anthropic" | "openai" | "gemini" | "openai-compatible";
const VENDORS: Record<Vendor, { name: string; endpoint: string; model: string; keyUrl: string }> = {
  anthropic: {
    name: "Anthropic (Claude)",
    endpoint: "https://api.anthropic.com/v1",
    model: "claude-3-5-haiku-latest",
    keyUrl: "https://console.anthropic.com/settings/keys",
  },
  openai: {
    name: "OpenAI",
    endpoint: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
    keyUrl: "https://platform.openai.com/api-keys",
  },
  gemini: {
    name: "Google Gemini",
    endpoint: "https://generativelanguage.googleapis.com/v1beta/openai",
    model: "gemini-2.0-flash",
    keyUrl: "https://aistudio.google.com/apikey",
  },
  "openai-compatible": { name: "OpenAI-compatible (custom)", endpoint: "", model: "", keyUrl: "" },
};
const vendorList = (Object.keys(VENDORS) as Vendor[]).map((id) => ({ id, name: VENDORS[id].name }));

// Commit messages want small, fast, cheap models. These patterns pick the good
// ones out of whatever the provider actually returns; the defaults are shown as
// chips before a fetch so there's always a sensible suggestion.
const REC_PATTERNS: Record<string, string[]> = {
  anthropic: ["haiku"],
  openai: ["mini", "nano"],
  gemini: ["flash"],
  "openai-compatible": ["haiku", "flash", "mini", "nano", "8b", "7b", "gemma", "small"],
  local: ["coder", "codellama", "codegemma", "deepseek", "qwen", "phi", "llama3.2", "gemma", "1.5b", "3b"],
};
const REC_DEFAULTS: Record<string, string[]> = {
  anthropic: ["claude-3-5-haiku-latest"],
  openai: ["gpt-4o-mini"],
  gemini: ["gemini-2.0-flash", "gemini-1.5-flash"],
  "openai-compatible": [],
  local: [],
};
const recKey = computed(() => (fType.value === "local" ? "local" : vendor.value));
const recommended = computed(() => {
  const key = recKey.value;
  if (models.value.length) {
    const pats = REC_PATTERNS[key] ?? [];
    return models.value.filter((m) => pats.some((p) => m.toLowerCase().includes(p))).slice(0, 8);
  }
  return REC_DEFAULTS[key] ?? [];
});

// add-form state
const fType = ref<"local" | "cloud">("cloud");
const vendor = ref<Vendor>("anthropic");
const label = ref("");
const endpoint = ref("");
const model = ref("");
const apiKey = ref("");
const models = ref<string[]>([]);
const fetching = ref(false);
const busy = ref(false);
const formError = ref<string | null>(null);

onMounted(async () => {
  await refreshAiConfig();
  envKeys.value = await detectEnvKeys().catch(() => []);
  if (!cfg.value.providers.length) startAdd();
});

function startAdd() {
  adding.value = true;
  editId.value = null;
  keyIsSet.value = false;
  fType.value = "cloud";
  onTypeChange();
  label.value = "";
  apiKey.value = "";
  models.value = [];
  formError.value = null;
}

async function startEdit(p: AiProvider) {
  adding.value = true;
  editId.value = p.id;
  fType.value = p.kind === "local" ? "local" : "cloud";
  vendor.value = (p.vendor in VENDORS ? p.vendor : "openai-compatible") as Vendor;
  label.value = p.label;
  endpoint.value = p.endpoint;
  model.value = p.model;
  apiKey.value = "";
  models.value = [];
  formError.value = null;
  keyIsSet.value = p.kind === "cloud" ? await hasApiKey(p.id).catch(() => false) : false;
}

function onTypeChange() {
  if (fType.value === "local") {
    endpoint.value = "http://localhost:11434";
    model.value = "";
  } else {
    applyVendor();
  }
}
function applyVendor() {
  const d = VENDORS[vendor.value];
  endpoint.value = d.endpoint;
  model.value = d.model;
  models.value = [];
}

async function fetchModels() {
  fetching.value = true;
  formError.value = null;
  try {
    models.value = await listProviderModels(
      fType.value,
      fType.value === "cloud" ? vendor.value : "",
      endpoint.value,
      fType.value === "cloud" ? apiKey.value : undefined,
      editId.value ?? undefined,
    );
    if (models.value.length && !models.value.includes(model.value)) model.value = models.value[0];
    if (!models.value.length) formError.value = "No models returned.";
  } catch (e) {
    formError.value = String(e);
  } finally {
    fetching.value = false;
  }
}

async function save() {
  formError.value = null;
  const isCloud = fType.value === "cloud";
  const editing = editId.value !== null;
  if (!model.value.trim()) return (formError.value = "Choose a model (Fetch models, or type one).");
  if (isCloud && !apiKey.value.trim() && !(editing && keyIsSet.value))
    return (formError.value = "Enter your API key.");
  if (!endpoint.value.trim()) return (formError.value = "Enter the base URL.");

  const provider: AiProvider = {
    id: editId.value ?? crypto.randomUUID(),
    kind: isCloud ? "cloud" : "local",
    vendor: isCloud ? vendor.value : "",
    label: label.value.trim() || (isCloud ? `${VENDORS[vendor.value].name} · ${model.value}` : `Ollama · ${model.value}`),
    model: model.value.trim(),
    endpoint: endpoint.value.trim().replace(/\/$/, ""),
  };
  // On edit: only send a key if the user typed a new one (blank keeps existing).
  const keyToSend = isCloud && apiKey.value.trim() ? apiKey.value : undefined;
  const makeDefault = editing ? false : cfg.value.providers.length === 0;

  busy.value = true;
  try {
    await saveAiProvider(provider, makeDefault, keyToSend);
    await refreshAiConfig();
    adding.value = false;
    editId.value = null;
    toast(editing ? "Provider updated" : "AI provider added", provider.label);
  } catch (e) {
    formError.value = String(e);
  } finally {
    busy.value = false;
  }
}

function getKey() {
  const url = VENDORS[vendor.value].keyUrl;
  if (url) openUrl(url);
}

async function useEnvKey(k: EnvKey) {
  const v = (k.vendor as Vendor) in VENDORS ? (k.vendor as Vendor) : "openai-compatible";
  const d = VENDORS[v];
  const provider: AiProvider = {
    id: crypto.randomUUID(),
    kind: "cloud",
    vendor: v,
    label: `${d.name} (from ${k.var})`,
    model: d.model || "gpt-4o-mini",
    endpoint: d.endpoint,
  };
  try {
    await saveAiProviderFromEnv(provider, k.var, cfg.value.providers.length === 0);
    await refreshAiConfig();
    envKeys.value = envKeys.value.filter((e) => e.var !== k.var);
    toast("Provider added from environment", k.var);
  } catch (e) {
    toast("Couldn't use env key", String(e), "error");
  }
}

async function connectOpenRouter() {
  orConnecting.value = true;
  try {
    const res = await openrouterLogin();
    await refreshAiConfig();
    orPick.value = { providerId: res.providerId, models: res.models, model: res.models[0] ?? "" };
    adding.value = false;
    toast("OpenRouter connected", "Pick a model to finish");
  } catch (e) {
    toast("OpenRouter login failed", String(e), "error");
  } finally {
    orConnecting.value = false;
  }
}

async function saveOrModel() {
  if (!orPick.value) return;
  const p = cfg.value.providers.find((x) => x.id === orPick.value!.providerId);
  if (p) {
    await saveAiProvider({ ...p, model: orPick.value.model }, true);
    await refreshAiConfig();
  }
  orPick.value = null;
}

async function makeDefault(id: string) {
  await setDefaultAiProvider(id);
  await refreshAiConfig();
}
async function remove(p: AiProvider) {
  if (!(await promptConfirm({ title: `Remove "${p.label}"?`, body: "Its Keychain key is deleted too.", confirmLabel: "Remove", danger: true }))) return;
  await removeAiProvider(p.id);
  await refreshAiConfig();
}
async function test(p: AiProvider) {
  testing.value = p.id;
  try {
    toast("Provider OK", await testAiProvider(p.id));
  } catch (e) {
    toast("Provider test failed", String(e), "error");
  } finally {
    testing.value = null;
  }
}

const host = (p: AiProvider) => p.endpoint.replace(/^https?:\/\//, "").split("/")[0];
</script>

<template>
  <div class="panel">
    <!-- Configured providers -->
    <div class="list" v-if="cfg.providers.length">
      <div v-for="p in cfg.providers" :key="p.id" class="prov">
        <span class="kind" :class="p.kind">{{ p.kind === "local" ? "LOCAL" : "CLOUD" }}</span>
        <div class="meta">
          <div class="label">
            <span class="lbl">{{ p.label }}</span>
            <span v-if="p.id === cfg.defaultId" class="default-tag">DEFAULT</span>
            <span v-if="!p.model" class="warn">needs a model</span>
          </div>
          <div class="sub mono">{{ p.model || "—" }} · {{ host(p) }}</div>
        </div>
        <div class="actions">
          <button class="mini" @click="startEdit(p)">Edit</button>
          <button class="mini" :disabled="testing === p.id" @click="test(p)">{{ testing === p.id ? "…" : "Test" }}</button>
          <button v-if="p.id !== cfg.defaultId" class="mini" @click="makeDefault(p.id)">Default</button>
          <button class="mini danger" @click="remove(p)">Remove</button>
        </div>
      </div>
    </div>

    <!-- OpenRouter finish-up: pick a model -->
    <div v-if="orPick" class="add">
      <div class="add-title section-label">OpenRouter connected — pick a model</div>
      <label class="field">
        <span>Model</span>
        <select v-model="orPick.model">
          <option v-for="m in orPick.models" :key="m" :value="m">{{ m }}</option>
        </select>
      </label>
      <div class="add-actions"><button class="btn-accent" @click="saveOrModel">Done</button></div>
    </div>

    <!-- Add / edit form -->
    <div v-else-if="adding" class="add">
      <div v-if="editId" class="add-title section-label">Edit provider</div>

      <!-- one-click + detected keys (add only) -->
      <template v-else>
        <button class="or-btn" :disabled="orConnecting" @click="connectOpenRouter">
          <span v-if="orConnecting" class="spinner-sm"></span>
          {{ orConnecting ? "Waiting for browser…" : "Connect with OpenRouter (one-click, all models)" }}
        </button>
        <div v-if="envKeys.length" class="detected">
          <span class="section-label">Found on this Mac</span>
          <button v-for="k in envKeys" :key="k.var" class="chip" @click="useEnvKey(k)">
            Use {{ k.var }} <span class="mono">{{ k.masked }}</span>
          </button>
        </div>
        <div class="or-sep"><span>or add manually</span></div>
      </template>

      <div class="seg">
        <button :class="{ on: fType === 'cloud' }" @click="fType = 'cloud'; onTypeChange()">Cloud API</button>
        <button :class="{ on: fType === 'local' }" @click="fType = 'local'; onTypeChange()">Local (Ollama)</button>
      </div>

      <template v-if="fType === 'cloud'">
        <label class="field">
          <span>Provider
            <a v-if="VENDORS[vendor].keyUrl" class="key-link" @click.prevent="getKey">Get a key ↗</a>
          </span>
          <select v-model="vendor" @change="applyVendor">
            <option v-for="v in vendorList" :key="v.id" :value="v.id">{{ v.name }}</option>
          </select>
        </label>
        <label class="field">
          <span v-if="editId && keyIsSet">API key <em>— leave blank to keep the saved key</em></span>
          <span v-else>API key <em>— stored in your macOS Keychain</em></span>
          <input
            v-model="apiKey"
            type="password"
            :placeholder="editId && keyIsSet ? 'saved in Keychain' : 'paste key'"
            spellcheck="false"
            autocomplete="off"
          />
        </label>
      </template>

      <label class="field">
        <span>Label</span>
        <input v-model="label" placeholder="optional" spellcheck="false" />
      </label>
      <label class="field">
        <span>Base URL</span>
        <input v-model="endpoint" spellcheck="false" :readonly="fType === 'cloud' && vendor !== 'openai-compatible'" />
      </label>
      <label class="field">
        <span>Model
          <a v-if="models.length" class="key-link" @click.prevent="models = []">type manually</a>
        </span>
        <div v-if="recommended.length" class="rec">
          <span class="rec-label">Recommended</span>
          <button
            v-for="m in recommended"
            :key="m"
            class="rec-chip"
            :class="{ on: m === model }"
            @click="model = m"
          >{{ m }}</button>
        </div>
        <div class="model-row">
          <select v-if="models.length" v-model="model">
            <option v-for="m in models" :key="m" :value="m">{{ m }}</option>
          </select>
          <input v-else v-model="model" spellcheck="false" placeholder="model name" />
          <button class="mini" :disabled="fetching" @click="fetchModels">{{ fetching ? "…" : "Fetch models" }}</button>
        </div>
        <div v-if="models.length" class="model-count mono">{{ models.length }} models available</div>
      </label>

      <p v-if="formError" class="form-error mono">{{ formError }}</p>
      <div class="add-actions">
        <button class="btn-accent" :disabled="busy" @click="save">
          {{ busy ? "Saving…" : editId ? "Save changes" : "Save provider" }}
        </button>
        <button v-if="cfg.providers.length" class="btn" @click="adding = false; editId = null">Cancel</button>
      </div>
    </div>

    <button v-else class="add-more" @click="startAdd">+ Add a provider</button>

    <div class="note">
      API keys are stored in your macOS Keychain, not in Plumb's config file.
    </div>
  </div>
</template>

<style scoped>
.panel { font-size: 13px; }
.list { display: flex; flex-direction: column; gap: 2px; margin-bottom: var(--space-4); }
.prov { display: flex; align-items: center; gap: var(--space-2); padding: var(--space-3); background: var(--raised); border: 1px solid var(--line); }
.kind { font-family: var(--font-mono); font-size: 9.5px; font-weight: 700; padding: 2px 5px; flex: none; }
.kind.local { border: 1px solid var(--lane-3); color: var(--lane-3); }
.kind.cloud { border: 1px solid var(--accent); color: var(--accent); }
.meta { min-width: 0; flex: 1; }
.label { font-size: 13px; font-weight: 600; display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
.lbl { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
.actions { display: flex; gap: 2px; flex: none; }
.default-tag { font-family: var(--font-mono); font-size: 9px; font-weight: 700; background: var(--accent); color: var(--accent-on); padding: 1px 4px; flex: none; }
.warn { font-family: var(--font-mono); font-size: 9px; color: var(--lane-2); border: 1px solid var(--lane-2); padding: 1px 4px; flex: none; }
.sub { font-size: 10.5px; color: var(--text-faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mini { font-size: 11px; padding: 4px 8px; background: var(--raised); border: 1px solid var(--line); cursor: pointer; flex: none; }
.mini.danger { color: var(--accent); }
.mini:disabled { opacity: 0.5; }

.add { border: 1px solid var(--line); padding: var(--space-4); background: var(--bg); }
.add-title { margin-bottom: var(--space-3); }
.or-btn { width: 100%; padding: 10px; background: var(--accent); color: var(--accent-on); border: 1px solid var(--accent); font-weight: 700; font-size: 12.5px; cursor: pointer; display: flex; align-items: center; justify-content: center; gap: var(--space-2); }
.or-btn:disabled { opacity: 0.7; }
.spinner-sm { width: 11px; height: 11px; border: 2px solid color-mix(in srgb, var(--accent-on) 40%, transparent); border-top-color: var(--accent-on); border-radius: 50%; animation: plumb-spin 0.7s linear infinite; }
@keyframes plumb-spin { to { transform: rotate(360deg); } }
.detected { margin-top: var(--space-3); display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-2); }
.chip { font-size: 11.5px; padding: 5px 10px; background: var(--raised); border: 1px solid var(--lane-3); color: var(--text); cursor: pointer; }
.chip .mono { color: var(--text-faint); font-size: 10px; }
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
.field input, .field select { height: 30px; padding: 0 10px; background: var(--surface); border: 1px solid var(--line); color: var(--text); font-size: 13px; }
.field input:focus, .field select:focus { outline: none; border-color: var(--accent); }
.field input[readonly] { color: var(--text-dim); }
.model-row { display: flex; gap: var(--space-2); }
.model-row input, .model-row select { flex: 1; }
.model-count { font-size: 10px; color: var(--text-faint); margin-top: 4px; }
.rec { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; margin-bottom: var(--space-2); }
.rec-label { font-size: 10px; letter-spacing: 0.1em; text-transform: uppercase; color: var(--text-faint); }
.rec-chip { font-family: var(--font-mono); font-size: 11px; padding: 3px 8px; background: var(--surface); border: 1px solid var(--line); color: var(--text-mid); cursor: pointer; }
.rec-chip:hover { border-color: var(--accent); color: var(--text); }
.rec-chip.on { background: var(--accent); color: var(--accent-on); border-color: var(--accent); }
.form-error { color: var(--accent); font-size: 11px; margin: 0 0 var(--space-3); }
.add-actions { display: flex; gap: var(--space-2); }
.btn-accent { height: 32px; padding: 0 16px; background: var(--accent); color: var(--accent-on); border: 1px solid var(--accent); font-weight: 700; font-size: 12.5px; cursor: pointer; }
.btn-accent:disabled { opacity: 0.5; }
.btn { height: 32px; padding: 0 16px; background: var(--raised); border: 1px solid var(--line); font-size: 12.5px; cursor: pointer; }
.add-more { width: 100%; padding: var(--space-3); background: var(--raised); border: 1px dashed var(--line); cursor: pointer; font-size: 12.5px; color: var(--text-mid); }
.note { margin-top: var(--space-4); font-size: 11px; color: var(--text-faint); line-height: 1.5; border-top: 1px solid var(--line); padding-top: var(--space-3); }
</style>
