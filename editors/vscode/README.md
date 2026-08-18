# Open in Plumb — VS Code extension

Adds an **Open in Plumb** command (Command Palette, SCM title bar, and the
Explorer right-click menu) that launches the [Plumb](https://github.com/Cyapow/plumb)
Git client at the current workspace folder.

Plumb is single-instance: if it's already open, this focuses its window and
switches to the repo instead of opening a second copy.

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
