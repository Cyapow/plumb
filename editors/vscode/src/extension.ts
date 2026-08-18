import * as vscode from "vscode";
import { spawn, type ChildProcess } from "child_process";

/**
 * Two ways to open the current workspace in Plumb:
 *  - "Open in Plumb"        launches the desktop app (single-instance).
 *  - "Open Plumb Panel"     runs `plumb serve` and hosts the real Plumb UI in
 *                           an editor tab via a webview.
 */
export function activate(context: vscode.ExtensionContext) {
  context.subscriptions.push(
    vscode.commands.registerCommand("plumb.openRepo", (uri?: vscode.Uri) => launchDesktop(uri)),
    vscode.commands.registerCommand("plumb.openPanel", (uri?: vscode.Uri) => openPanel(context, uri)),
  );
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

/** Launch the standalone desktop app at the folder (single-instance focuses it). */
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

/** Start `plumb serve <folder>` and resolve once it prints its port + token. */
function spawnServe(bin: string, folder: string): Promise<{ proc: ChildProcess; port: number; token: string }> {
  return new Promise((resolve, reject) => {
    const proc = spawn(bin, ["serve", folder], { stdio: ["ignore", "pipe", "pipe"] });
    let buf = "";
    const timer = setTimeout(() => {
      proc.kill();
      reject(new Error("timed out waiting for `plumb serve` to start"));
    }, 15000);
    proc.stdout?.on("data", (d: Buffer) => {
      buf += d.toString();
      const m = buf.match(/PLUMB_SERVE port=(\d+) token=(\w+)/);
      if (m) {
        clearTimeout(timer);
        resolve({ proc, port: Number(m[1]), token: m[2] });
      }
    });
    proc.on("error", (e) => {
      clearTimeout(timer);
      reject(e);
    });
    proc.on("exit", (code) => {
      clearTimeout(timer);
      reject(new Error(`\`plumb serve\` exited (${code}). Is this a build that supports serve mode?`));
    });
  });
}

async function openPanel(_context: vscode.ExtensionContext, uri?: vscode.Uri) {
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

  panel.webview.html = `<!DOCTYPE html><html><body style="margin:0;background:#161514;color:#888;font-family:sans-serif">
    <div style="padding:24px">Starting Plumb…</div></body></html>`;

  let proc: ChildProcess | undefined;
  try {
    const started = await spawnServe(bin, folder);
    proc = started.proc;
    // asExternalUri makes the local port reachable from the webview, including
    // over Remote / Codespaces where 127.0.0.1 alone wouldn't resolve.
    const external = await vscode.env.asExternalUri(vscode.Uri.parse(`http://127.0.0.1:${started.port}`));
    const src = external.toString();
    const origin = `${external.scheme}://${external.authority}`;
    panel.webview.html = `<!DOCTYPE html><html><head><meta charset="utf-8">
      <meta http-equiv="Content-Security-Policy" content="default-src 'none'; frame-src ${origin}; style-src 'unsafe-inline';">
      <style>html,body{margin:0;height:100%}iframe{border:0;width:100%;height:100vh;display:block}</style>
      </head><body><iframe src="${src}"></iframe></body></html>`;
  } catch (err) {
    panel.webview.html = `<!DOCTYPE html><html><body style="margin:0;background:#161514;color:#e0663a;font-family:sans-serif">
      <div style="padding:24px">Couldn't start Plumb: ${escapeHtml(String(err))}<br><br>
      Set <code>plumb.binaryPath</code> to a Plumb build that supports <code>serve</code>.</div></body></html>`;
  }

  panel.onDidDispose(() => proc?.kill());
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!));
}

export function deactivate() {}
