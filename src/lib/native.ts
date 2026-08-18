// Native-capability shim.
//
// Opening links, revealing files, and folder pickers are Tauri plugin calls,
// not `invoke` commands, so they don't work in served mode (browser / editor
// panel). Route them here: the Tauri plugin on the desktop, and — since the
// serve agent runs on the same machine — a server-side action (or a path
// prompt) when served. Call sites import from here instead of the plugins.
import { openUrl as tauriOpenUrl, revealItemInDir as tauriReveal } from "@tauri-apps/plugin-opener";
import { open as tauriDialogOpen } from "@tauri-apps/plugin-dialog";
import { invoke, isServed } from "./transport";
import { promptText } from "./ui";

/** Open a URL (or file) in the OS default handler. */
export function openUrl(url: string): Promise<unknown> {
  return isServed ? invoke("open_url", { url }) : tauriOpenUrl(url);
}

/** Reveal a file/folder in the OS file manager. */
export function revealItemInDir(path: string): Promise<unknown> {
  return isServed ? invoke("reveal_path", { path }) : tauriReveal(path);
}

/** Pick a folder. Served mode has no native picker, so it prompts for a path. */
export async function openFolder(title = "Open folder"): Promise<string | null> {
  if (isServed) {
    const p = await promptText({ title, label: "Folder path", placeholder: "/path/to/folder" });
    return p && p.trim() ? p.trim() : null;
  }
  const res = await tauriDialogOpen({ directory: true, multiple: false, title });
  return typeof res === "string" ? res : null;
}
