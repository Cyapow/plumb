# Plumb — JetBrains plugin

Hosts the [Plumb](https://github.com/Cyapow/plumb) Git client in a **tool window**
(right dock) via JCEF, backed by a shared local `plumb serve` agent. The agent
is reused across projects/windows and started automatically if not already
running — the same discovery mechanism the VS Code extension uses.

Requires a Plumb build that supports serve mode (**v0.10.7+**).

## Install

**From a release (no toolchain needed).** Download
`plumb-jetbrains-<version>.zip` from the
[latest release](https://github.com/Cyapow/plumb/releases/latest), then in your
IDE: **Settings → Plugins → ⚙ → Install Plugin from Disk…** and pick the zip.
Open the **Plumb** tool window (right dock).

**From source** — see *Build & run* below.

## Build & run

Needs a JDK 17 and Gradle (or generate the wrapper once with `gradle wrapper`).

```bash
cd editors/jetbrains
gradle runIde          # launches a sandbox IDE with the plugin loaded
# or
gradle buildPlugin     # produces build/distributions/plumb-jetbrains-0.1.0.zip
```

Install the zip via **Settings → Plugins → ⚙ → Install Plugin from Disk…**.

Open the **Plumb** tool window (right edge) in any project — it starts/reuses the
agent and shows Plumb for that project.

## Binary location

The plugin looks for the Plumb binary at the platform default:

- macOS: `/Applications/Plumb.app/Contents/MacOS/plumb`
- Linux: `plumb` on `PATH`
- Windows: `plumb.exe`

Override with the **`PLUMB_BIN`** environment variable (point it at the binary if
it's installed elsewhere).

## Notes

- Requires a JCEF-enabled IDE build (the default for current IntelliJ-based IDEs).
- The panel loads the served frontend directly; the session token is injected
  server-side and never appears in a URL.
