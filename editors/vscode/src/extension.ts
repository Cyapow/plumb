import * as vscode from "vscode";
import { spawn } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as http from "http";

/**
 * Two ways to open the current workspace in Plumb:
 *  - "Open in Plumb (desktop)" launches the standalone desktop app.
 *  - "Open Plumb Panel"        hosts the real Plumb UI in an editor tab, backed
 *                              by a shared `plumb serve` agent (reused across
 *                              panels/windows; spawned once if not running).
 */
export function activate(context: vscode.ExtensionContext) {
  context.subscriptions.push(
    vscode.commands.registerCommand("plumb.openRepo", (uri?: vscode.Uri) => launchDesktop(uri)),
    vscode.commands.registerCommand("plumb.openPanel", (uri?: vscode.Uri) => openPanel(uri)),
  );

  // A one-click launcher in the status bar, so the panel opens without the
  // Command Palette.
  const status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0);
  status.text = "$(git-branch) Plumb";
  status.tooltip = "Open the Plumb panel";
  status.command = "plumb.openPanel";
  status.show();
  context.subscriptions.push(status);
}

function workspaceFolder(uri?: vscode.Uri): string | undefined {
  return uri?.fsPath ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? undefined;
}

function binaryPath(): string {
  const configured = vscode.workspace.getConfiguration("plumb").get<string>("binaryPath")?.trim();
  if (configured) return configured;
  switch (process.platform) {
    case "darwin":
      return "/Applications/Plumb.app/Contents/MacOS/plumb";
    case "win32":
      return "plumb.exe";
    default:
      return "plumb";
  }
}

function launchDesktop(uri?: vscode.Uri) {
  const folder = workspaceFolder(uri);
  if (!folder) {
    vscode.window.showWarningMessage("Plumb: open a folder or workspace first.");
    return;
  }
  const bin = binaryPath();
  try {
    const child = spawn(bin, [folder], { detached: true, stdio: "ignore" });
    child.on("error", (err) =>
      vscode.window.showErrorMessage(`Plumb: couldn't launch "${bin}". Set "plumb.binaryPath" in settings. (${err.message})`),
    );
    child.unref();
  } catch (err) {
    vscode.window.showErrorMessage(`Plumb: ${String(err)}`);
  }
}

/* ── Shared serve agent ── */

function discoveryPath(): string {
  if (process.platform === "darwin") return path.join(os.homedir(), "Library/Application Support/plumb/serve.json");
  if (process.platform === "win32") return path.join(process.env.APPDATA || os.homedir(), "plumb", "serve.json");
  return path.join(process.env.XDG_CONFIG_HOME || path.join(os.homedir(), ".config"), "plumb", "serve.json");
}

function readDiscovery(): { port: number; token: string; pid: number } | null {
  try {
    return JSON.parse(fs.readFileSync(discoveryPath(), "utf8"));
  } catch {
    return null;
  }
}

function pidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function health(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const req = http.get({ host: "127.0.0.1", port, path: "/", timeout: 800 }, (r) => {
      r.resume();
      resolve((r.statusCode ?? 500) < 500);
    });
    req.on("error", () => resolve(false));
    req.on("timeout", () => {
      req.destroy();
      resolve(false);
    });
  });
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Return the port of a live agent, reusing one if advertised, else spawning it. */
async function ensureServer(bin: string, folder: string): Promise<number> {
  const disc = readDiscovery();
  if (disc && pidAlive(disc.pid) && (await health(disc.port))) return disc.port;

  // Spawn a detached, persistent agent; it advertises itself in the discovery
  // file, which we then wait for.
  const child = spawn(bin, ["serve", folder], { detached: true, stdio: "ignore" });
  let spawnError: Error | undefined;
  child.on("error", (e) => (spawnError = e));
  child.unref();

  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    if (spawnError) throw spawnError;
    const d = readDiscovery();
    if (d && pidAlive(d.pid) && (await health(d.port))) return d.port;
    await sleep(300);
  }
  throw new Error("`plumb serve` didn't start. Set `plumb.binaryPath` to a build that supports serve mode.");
}

async function openPanel(uri?: vscode.Uri) {
  const folder = workspaceFolder(uri);
  if (!folder) {
    vscode.window.showWarningMessage("Plumb: open a folder or workspace first.");
    return;
  }
  const bin = binaryPath();

  const panel = vscode.window.createWebviewPanel("plumb.panel", "Plumb", vscode.ViewColumn.Active, {
    enableScripts: true,
    retainContextWhenHidden: true,
  });
  panel.webview.html = shellHtml("Starting Plumb…", "#888");

  try {
    const port = await ensureServer(bin, folder);
    // asExternalUri makes the local port reachable from the webview, including
    // over Remote / Codespaces. This panel opens its own repo via ?repo=.
    const external = await vscode.env.asExternalUri(vscode.Uri.parse(`http://127.0.0.1:${port}`));
    const origin = `${external.scheme}://${external.authority}`;
    const src = `${external.toString().replace(/\/$/, "")}/?repo=${encodeURIComponent(folder)}`;
    panel.webview.html = `<!DOCTYPE html><html><head><meta charset="utf-8">
      <meta http-equiv="Content-Security-Policy" content="default-src 'none'; frame-src ${origin}; style-src 'unsafe-inline';">
      <style>html,body{margin:0;height:100%}iframe{border:0;width:100%;height:100vh;display:block}</style>
      </head><body><iframe src="${src}"></iframe></body></html>`;
  } catch (err) {
    panel.webview.html = shellHtml(`Couldn't start Plumb: ${escapeHtml(String(err))}`, "#e0663a");
  }
  // The agent is shared and persistent — closing a panel does not stop it.
}

function shellHtml(message: string, color: string): string {
  return `<!DOCTYPE html><html><body style="margin:0;background:#161514;color:${color};font-family:sans-serif">
    <div style="padding:24px">${escapeHtml(message)}</div></body></html>`;
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!));
}

export function deactivate() {}
