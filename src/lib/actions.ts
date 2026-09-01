// Custom Actions: user-defined commands surfaced in the toolbar and context
// menus. Definitions live in the backend config; the frontend triggers them by
// id and shows the output.
import { reactive } from "vue";
import { invoke } from "./transport";
import { toast, promptConfirm } from "./ui";

export type ActionContext = "toolbar" | "commit" | "branch" | "file";

export interface CustomAction {
  id: string;
  label: string;
  program: string;
  args: string[];
  context: ActionContext;
  confirm: boolean;
}

export interface ActionCtx {
  sha?: string;
  shortSha?: string;
  branch?: string;
  file?: string;
}

export function listActions(): Promise<CustomAction[]> {
  return invoke("list_actions");
}
export function saveActions(actions: CustomAction[]): Promise<void> {
  return invoke("save_actions", { actions });
}
export function runAction(repoPath: string, id: string, ctx: ActionCtx): Promise<string> {
  return invoke("run_action", { repoPath, id, ctx });
}

// Shared store so the toolbar and every context menu see the same list.
export const actionsStore = reactive<{ list: CustomAction[] }>({ list: [] });
export async function refreshActions() {
  try {
    actionsStore.list = await listActions();
  } catch {
    actionsStore.list = [];
  }
}
export const actionsFor = (ctx: ActionContext) => actionsStore.list.filter((a) => a.context === ctx);

/** Run an action (with optional confirm) and surface its output as a toast. */
export async function invokeAction(a: CustomAction, repoPath: string, ctx: ActionCtx = {}) {
  if (a.confirm && !(await promptConfirm({ title: `Run "${a.label}"?`, confirmLabel: "Run" }))) return;
  try {
    const out = await runAction(repoPath, a.id, ctx);
    toast(a.label, out);
  } catch (e) {
    toast(a.label, String(e), "error");
  }
}

function randomId(): string {
  return "act-" + Math.random().toString(36).slice(2, 10);
}
export function blankAction(): CustomAction {
  return { id: randomId(), label: "", program: "", args: [], context: "toolbar", confirm: false };
}
