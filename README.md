<div align="center">

# Plumb

**A straight line through your history.**

A free, native macOS Git client — a leaner, faster alternative to GitKraken.
No account, no trial, no upsell. Built with Tauri + Vue.

</div>

---

## Features

**Core Git**
- Commit graph (DAG) with hard-diagonal lane routing and a colorblind-safe palette
- Stage by **file, hunk, or individual lines**; amend, sign-off, and **GPG/SSH signing**
- Commit / **undo–redo**; fetch / pull / push over your existing SSH keys
- Pull options (merge / rebase / fast-forward-only) and push options (force-with-lease, tags, set-upstream) on right-click
- Branches in a **collapsible tree** (local + remote): check out, create-here, rename, delete, remote-branch checkout
- **Merge, rebase, cherry-pick, revert** with a real **3-way conflict editor** (take a side or hand-edit)
- **Interactive rebase** — reorder / squash / fixup / drop; **reword** commit messages
- **Reflog** — recover lost commits after a bad reset or rebase
- **Compare** any two branches/commits, and **search all history** by message or code change (pickaxe)
- **Stashes**, **tags**, **clone** (browse your accounts' repos or paste a URL), **init** & **publish** empty repos
- **Submodules**, **worktrees**, and **bisect**
- Syntax-highlighted diffs with **word-level** emphasis and an optional **side-by-side** view; full-screen viewer
- **File history** and **blame**; **open in editor** / **add to .gitignore** from context menus
- Right-click menus everywhere, a native **menu bar**, and a **⌘K command palette**
- **Auto-refresh** file watcher, and **reopen-where-you-left-off** on launch

**Multi-account (the differentiator)**
- Connect **multiple GitHub and GitLab accounts** — including Enterprise / self-managed
- **OAuth one-click** (GitHub Device Flow, GitLab PKCE) *or* personal access tokens
- Tokens stored in the **macOS Keychain**, never on disk
- **Create and view pull / merge requests** for the open repo (filterable by Created / Assigned / Reviewing)

**CI / pipelines**
- CI status **badges on PRs and on commits** (GitHub Checks / GitLab pipelines)
- **Pipeline detail** — jobs/stages, open logs, **retry / cancel**
- **Run pipelines** from the app (GitLab pipeline, GitHub `workflow_dispatch`)
- **Desktop notifications** when a pipeline finishes

**AI commits (bring your own model)**
- **Generate commit messages** from the staged diff; **split** a messy tree into focused commits
- Pluggable providers: **Local (Ollama)**, **OpenAI**, **Anthropic**, **Gemini**, **OpenRouter**
- API keys in the Keychain; local models never touch the network

**Make it yours**
- 9 built-in **themes** (Modernist, Catppuccin ×4, Material ×3) with per-theme code colors, plus **custom themes**
- Bundled code **fonts** (JetBrains Mono / Fira Code / IBM Plex Mono) or any font on your system; adjustable size & line height

## Download

Grab the latest `Plumb_*_universal.dmg` from the [**Releases**](../../releases) page (universal — Apple Silicon + Intel).

Because there's no Apple Developer account, the build isn't notarized, so macOS Gatekeeper
blocks it on first launch. To open it:

1. Move **Plumb.app** to `/Applications` and try to open it once.
2. Go to **System Settings → Privacy & Security**, scroll down, and click **Open Anyway**.

Or clear the quarantine flag from Terminal:

```bash
xattr -dr com.apple.quarantine /Applications/Plumb.app
```

## Build from source

Prerequisites: **Rust** (`rustup`), **Node 20+**, and Xcode Command Line Tools.

```bash
npm install
npm run tauri dev      # run in development
npm run tauri build    # produce a release .app / .dmg in src-tauri/target/release/bundle
```

## Setup notes

**AI providers** — open **⚙ → AI providers**:
- *Local:* install [Ollama](https://ollama.com) (`ollama pull qwen2.5-coder`), then "Detect".
- *Cloud:* paste an API key (or use auto-detected env vars), or **Connect with OpenRouter**.

**Accounts** — open **⚙ → Accounts**. Personal access tokens work everywhere
(GitHub scopes `repo, read:user`; GitLab scope `api`). For one-click OAuth, register a small
OAuth app once and paste its public client ID:
- **GitHub:** create an OAuth App, enable *Device Flow*.
- **GitLab:** create an Application with redirect URI `http://127.0.0.1:47823/callback`, scope `api`.

## Editor / CLI integration

Plumb can be opened at a repo from the terminal or your editor. It's
single-instance, so a second launch focuses the running window and switches to
the repo.

**CLI** — `plumb [path]` opens a repository (defaults to the current directory):
- *Linux:* the `.deb` installs `plumb` on your `PATH` — just run `plumb .`.
- *macOS:* install the shim once — `install -m 0755 scripts/plumb /usr/local/bin/plumb` — then `plumb .`.
- *Windows:* add the install directory to `PATH`, then `plumb .`.

**VS Code** — the extension in [`editors/vscode`](editors/vscode) adds an
**Open in Plumb** command (Command Palette, SCM title bar, Explorer right-click).
See its README to build/install.

**JetBrains** — add an External Tool: Program = the Plumb binary (or the `plumb`
shim), Arguments = `$ProjectFileDir$`.

## Architecture

- **Frontend:** Vue 3 + TypeScript (Vite). Design system in `src/styles/`.
- **Backend:** Rust via Tauri v2. Local Git through [`git2`](https://docs.rs/git2) (libgit2);
  **network Git (fetch/pull/push/clone) shells out to your system `git`** so it uses your real
  SSH config, agent, and `known_hosts`.
- **Secrets:** macOS Keychain via the `keyring` crate. **HTTP:** `ureq` for provider/host APIs.

## Development

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # Rust unit tests
npx vue-tsc --noEmit                               # type-check the frontend
```

CI runs both on every push and pull request (`.github/workflows/ci.yml`).

**Skipping Keychain prompts while debugging.** Each rebuild re-signs the binary,
so macOS treats it as a new app and re-asks to authorize Keychain access. Set
`PLUMB_DEV_SECRETS=1` to store AI keys and account tokens in a `0600` JSON file
(`~/.plumb-dev-secrets.json`) instead of the Keychain — no prompts. It's a
plaintext dev-only store; never set it for a build you distribute.

## Releasing

Releases are cut with the version-bump helper, which updates every manifest, tags
`plumb-v*`, and pushes — the tag triggers a universal macOS build that publishes to Releases
(`.github/workflows/release.yml`):

```bash
npm run release -- patch    # or: minor | major | X.Y.Z
```

## License

MIT — see [LICENSE](LICENSE).
