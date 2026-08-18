// Typed wrappers over the Rust `ai` commands.
import { invoke } from "@tauri-apps/api/core";

export interface AiProvider {
  id: string;
  kind: string; // "local" | "cloud"
  vendor: string; // "" for local; "anthropic" | "openai" | "openai-compatible"
  label: string;
  model: string;
  endpoint: string;
}

export interface AiConfig {
  providers: AiProvider[];
  defaultId: string | null;
}

export interface GeneratedMessage {
  message: string;
  providerLabel: string;
  model: string;
  isLocal: boolean;
  host: string;
  files: number;
  added: number;
  removed: number;
  ms: number;
}

export function listAiProviders(): Promise<AiConfig> {
  return invoke("list_ai_providers");
}

export function saveAiProvider(
  provider: AiProvider,
  makeDefault: boolean,
  apiKey?: string,
): Promise<AiConfig> {
  return invoke("save_ai_provider", { provider, makeDefault, apiKey: apiKey ?? null });
}

export function removeAiProvider(id: string): Promise<AiConfig> {
  return invoke("remove_ai_provider", { id });
}

export function setDefaultAiProvider(id: string): Promise<AiConfig> {
  return invoke("set_default_ai_provider", { id });
}

export function hasApiKey(id: string): Promise<boolean> {
  return invoke("has_api_key", { id });
}

export function testAiProvider(id: string): Promise<string> {
  return invoke("test_ai_provider", { id });
}

export function listOllamaModels(endpoint: string): Promise<string[]> {
  return invoke("list_ollama_models", { endpoint });
}

export function listProviderModels(
  kind: string,
  vendor: string,
  endpoint: string,
  apiKey?: string,
  providerId?: string,
): Promise<string[]> {
  return invoke("list_provider_models", {
    kind,
    vendor,
    endpoint,
    apiKey: apiKey ?? null,
    providerId: providerId ?? null,
  });
}

export interface EnvKey {
  var: string;
  vendor: string;
  masked: string;
}

export function detectEnvKeys(): Promise<EnvKey[]> {
  return invoke("detect_env_keys");
}

export function saveAiProviderFromEnv(
  provider: AiProvider,
  envVar: string,
  makeDefault: boolean,
): Promise<AiConfig> {
  return invoke("save_ai_provider_from_env", { provider, envVar, makeDefault });
}

export interface OpenRouterResult {
  providerId: string;
  models: string[];
}

export function openrouterLogin(): Promise<OpenRouterResult> {
  return invoke("openrouter_login");
}

export function generateCommitMessage(
  repoPath: string,
  providerId: string | null,
  conventional: boolean,
  style: "normal" | "shorter" | "detailed",
): Promise<GeneratedMessage> {
  return invoke("generate_commit_message", { repoPath, providerId, conventional, style });
}

/** Plain-language explanation of a commit's diff (or the working diff when sha omitted). */
export function explainDiff(repoPath: string, sha?: string, providerId?: string | null): Promise<string> {
  return invoke("explain_diff", { repoPath, sha: sha ?? null, providerId: providerId ?? null });
}

export interface CommitGroup {
  message: string;
  files: string[];
}
export function aiGroupChanges(
  repoPath: string,
  providerId: string | null,
  conventional: boolean,
): Promise<CommitGroup[]> {
  return invoke("ai_group_changes", { repoPath, providerId, conventional });
}
