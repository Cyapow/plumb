// Typed wrappers over the Rust `accounts` commands.
import { invoke } from "@tauri-apps/api/core";

export interface Connection {
  id: string;
  provider: string; // "github" | "gitlab"
  label: string;
  baseUrl: string;
  username: string;
  avatarUrl: string;
}

export interface ConnectionConfig {
  connections: Connection[];
}

export function listConnections(): Promise<ConnectionConfig> {
  return invoke("list_connections");
}

export function connectAccount(
  provider: string,
  baseUrl: string,
  token: string,
  label?: string,
): Promise<Connection> {
  return invoke("connect_account", { provider, baseUrl, token, label: label ?? null });
}

export function removeConnection(id: string): Promise<ConnectionConfig> {
  return invoke("remove_connection", { id });
}

export function testConnection(id: string): Promise<string> {
  return invoke("test_connection", { id });
}

export interface DeviceCode {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  interval: number;
}

export function githubDeviceStart(clientId: string): Promise<DeviceCode> {
  return invoke("github_device_start", { clientId });
}

export function githubDevicePoll(
  clientId: string,
  deviceCode: string,
  interval: number,
): Promise<Connection> {
  return invoke("github_device_poll", { clientId, deviceCode, interval });
}

export function gitlabOauthLogin(clientId: string): Promise<Connection> {
  return invoke("gitlab_oauth_login", { clientId });
}

export interface PullRequest {
  number: number;
  title: string;
  author: string;
  authorAvatar: string;
  draft: boolean;
  sourceBranch: string;
  targetBranch: string;
  url: string;
  updatedAt: string;
  provider: string;
  assignees: string[];
  reviewers: string[];
  ciStatus: string; // "success" | "failure" | "pending" | ""
  headSha: string;
}

export interface PrList {
  status: string; // "ok" | "no_remote" | "no_account"
  provider: string | null;
  host: string | null;
  username: string | null;
  items: PullRequest[];
}

export function listPullRequests(repoPath: string): Promise<PrList> {
  return invoke("list_pull_requests", { repoPath });
}

export interface CiStatus {
  sha: string;
  status: string; // "success" | "failure" | "pending"
}
export function listCiStatuses(repoPath: string): Promise<CiStatus[]> {
  return invoke("list_ci_statuses", { repoPath });
}

export interface WorkflowRef {
  id: string;
  name: string;
}
export function listWorkflows(repoPath: string): Promise<WorkflowRef[]> {
  return invoke("list_workflows", { repoPath });
}
export function triggerPipeline(repoPath: string, gitRef: string, workflowId?: string): Promise<string> {
  return invoke("trigger_pipeline", { repoPath, gitRef, workflowId: workflowId ?? null });
}

export interface PipelineJob {
  name: string;
  stage: string;
  status: string;
  webUrl: string;
}
export interface PipelineDetail {
  id: string;
  name: string;
  status: string;
  webUrl: string;
  jobs: PipelineJob[];
}
export function pipelineDetail(repoPath: string, sha: string): Promise<PipelineDetail[]> {
  return invoke("pipeline_detail", { repoPath, sha });
}
export function pipelineAction(repoPath: string, id: string, action: "retry" | "cancel"): Promise<string> {
  return invoke("pipeline_action", { repoPath, id, action });
}

export interface PrTarget {
  provider: string; // "github" | "gitlab" | ""
  host: string;
  repo: string;
}
export function prTarget(repoPath: string): Promise<PrTarget> {
  return invoke("pr_target", { repoPath });
}

export interface CreatedPr {
  url: string;
  number: number;
  provider: string;
}
export function createPullRequest(
  repoPath: string,
  opts: { sourceBranch: string; targetBranch: string; title: string; body: string; draft: boolean },
): Promise<CreatedPr> {
  return invoke("create_pull_request", {
    repoPath,
    sourceBranch: opts.sourceBranch,
    targetBranch: opts.targetBranch,
    title: opts.title,
    body: opts.body,
    draft: opts.draft,
  });
}

export interface RepoRef {
  name: string;
  sshUrl: string;
  httpUrl: string;
  description: string;
}

export function listAccountRepos(connectionId: string): Promise<RepoRef[]> {
  return invoke("list_account_repos", { connectionId });
}

export function createRemoteRepo(
  connectionId: string,
  name: string,
  isPrivate: boolean,
): Promise<RepoRef> {
  return invoke("create_remote_repo", { connectionId, name, private: isPrivate });
}
