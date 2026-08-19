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

Needs **JDK 17** (the IntelliJ Platform Gradle plugin doesn't support newer JDKs
here). The Gradle wrapper is committed and pinned to a compatible Gradle, so a
system Gradle isn't needed — use `./gradlew`.

```bash
cd editors/jetbrains
# point Gradle at a JDK 17 (Homebrew example):
export JAVA_HOME="$(brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"

./gradlew buildPlugin --no-daemon   # -> build/distributions/plumb-jetbrains-<version>.zip
# or, to try it in a throwaway sandbox IDE:
./gradlew runIde --no-daemon
```

Install the produced zip via **Settings → Plugins → ⚙ → Install Plugin from
Disk…**, then restart. If you're re-installing after a change, uninstall the old
Plumb plugin first (or bump `version` in `build.gradle.kts`) — JetBrains skips a
same-version install.

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
