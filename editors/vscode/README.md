# Plumb — VS Code extension

Two commands (Command Palette, SCM title bar, Explorer right-click):

- **Open Plumb Panel** — runs `plumb serve` for the workspace folder and hosts
  the real Plumb UI in an editor tab (a webview). The panel talks to the local
  server; the token is injected server-side and never travels in a URL. Uses
  `asExternalUri`, so it also works over Remote / Codespaces.
- **Open in Plumb (desktop)** — launches the standalone desktop app at the
  folder. Plumb is single-instance, so it focuses the existing window.

Both need a Plumb binary that supports `serve` (v0.10.7+) for the panel.

## Build & install (local)

```bash
cd editors/vscode
npm install
npm run compile
```

Then either press **F5** in VS Code to launch an Extension Development Host, or
package it:

```bash
npx @vscode/vsce package
code --install-extension plumb-vscode-0.1.0.vsix
```

## Settings

- `plumb.binaryPath` — path to the Plumb binary. Leave empty to use the default:
  - macOS: `/Applications/Plumb.app/Contents/MacOS/plumb`
  - Linux: `plumb` on your `PATH` (installed by the `.deb`)
  - Windows: `plumb.exe`

If the default isn't found, set this to the full path of the Plumb binary.
