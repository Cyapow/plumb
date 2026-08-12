<div align="center">

# Plumb

**A straight line through your history.**

A free, native macOS Git client — a leaner, faster alternative to GitKraken.
No account, no trial, no upsell. Built with Tauri + Vue.

</div>

---

## Features

**Core Git**
- Commit graph (DAG) with hard-diagonal lane routing, colorblind-safe palette
- Stage by **file, hunk, or individual lines**; amend & sign-off
- Commit / **undo–redo**; fetch / pull / push (over your existing SSH keys)
- Branches in a **collapsible tree** (local + remote): check out, create, rename, delete
- **Merge, rebase, cherry-pick, revert** with an in-app conflict banner (resolve → continue / abort)
- **Stashes** (save / apply / pop / drop), **tags**, **clone** (browse your accounts' repos or paste a URL)
- Syntax-highlighted diffs with **word-level** change emphasis; full-screen diff viewer
- **File history** and **blame**
- Right-click context menus everywhere; **⌘K command palette** (branches, commits, files, actions)
- **Auto-refresh** — watches the repo and updates when you edit files or run git in the terminal

**Multi-account (the differentiator)**
- Connect **multiple GitHub and GitLab accounts** — including Enterprise / self-managed
- **OAuth one-click** (GitHub Device Flow, GitLab PKCE) *or* personal access tokens
- Tokens stored in the **macOS Keychain**, never on disk
- **Pull / merge requests** for the open repo, filterable by *Created / Assigned / Reviewing*

**AI commits (bring your own model)**
- **Generate commit messages** from the staged diff
- **Split** a messy working tree into several focused commits, each with its own message
- Pluggable providers: **Local (Ollama)**, **OpenAI**, **Anthropic**, **Gemini**, **OpenRouter** (one-click)
- API keys in the Keychain; local models never touch the network

## Download

Grab the latest `Plumb_*_universal.dmg` from the [**Releases**](../../releases) page.

Because there's no Apple Developer account, the build isn't notarized. On first launch
macOS will say it's from an unidentified developer — to open it:

- **Right-click Plumb.app → Open → Open**, or
- ```bash
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
- *Cloud:* paste an API key (or use existing env vars auto-detected), or **Connect with OpenRouter** for one-click access to many models.

**Accounts** — open **⚙ → Accounts**. Personal access tokens work everywhere
(GitHub scopes `repo, read:user`; GitLab scope `api`). For one-click OAuth you
register a small OAuth app once and paste its public client ID:
- **GitHub:** create an OAuth App, enable *Device Flow*.
- **GitLab:** create an Application with redirect URI `http://127.0.0.1:47823/callback`,
  scope `api`, *Confidential* unchecked.

## Architecture

- **Frontend:** Vue 3 + TypeScript (Vite). Design system in `src/styles/`.
- **Backend:** Rust via Tauri v2. Local Git through the [`git2`](https://docs.rs/git2) (libgit2) crate;
  **network Git (fetch/pull/push/clone) shells out to your system `git`** so it uses your
  real SSH config, agent, and `known_hosts`.
- **Secrets:** macOS Keychain via the `keyring` crate.
- **HTTP:** `ureq` for provider/host APIs.

## Releasing

Pushing a tag builds and publishes a universal macOS app automatically
(see `.github/workflows/release.yml`):

```bash
git tag v0.1.0
git push origin v0.1.0
```

## License

MIT — see [LICENSE](LICENSE).
