import * as vscode from "vscode";
import { spawn } from "child_process";

/**
 * "Open in Plumb": launch the Plumb Git client at the current workspace folder.
 * Plumb is single-instance, so if it's already running this just focuses its
 * window and switches to the repo.
 */
export function activate(context: vscode.ExtensionContext) {
  const cmd = vscode.commands.registerCommand("plumb.openRepo", async (uri?: vscode.Uri) => {
    const folder =
      uri?.fsPath ??
      vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ??
      undefined;
    if (!folder) {
      vscode.window.showWarningMessage("Plumb: open a folder or workspace first.");
      return;
    }

    const configured = vscode.workspace.getConfiguration("plumb").get<string>("binaryPath")?.trim();
    const bin = configured || defaultBinary();

    try {
      const child = spawn(bin, [folder], { detached: true, stdio: "ignore" });
      child.on("error", (err) =>
        vscode.window.showErrorMessage(
          `Plumb: couldn't launch "${bin}". Install the CLI shim or set "plumb.binaryPath" in settings. (${err.message})`,
        ),
      );
      child.unref();
    } catch (err) {
      vscode.window.showErrorMessage(`Plumb: ${String(err)}`);
    }
  });
  context.subscriptions.push(cmd);
}

/** Best-guess binary when the user hasn't set one. */
function defaultBinary(): string {
  switch (process.platform) {
    case "darwin":
      return "/Applications/Plumb.app/Contents/MacOS/plumb";
    case "win32":
      return "plumb.exe";
    default:
      return "plumb"; // Linux .deb installs `plumb` on PATH
  }
}

export function deactivate() {}
