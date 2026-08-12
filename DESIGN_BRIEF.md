# Design Brief — Mac-native Git Client

> Paste this into Claude Design. Your job: propose a **product name** and a complete **visual design system** (naming, logo direction, color, typography, iconography, and key screens) for the app described below.

## What it is

A **free, macOS-native desktop Git client** — a graphical app for managing Git repositories: viewing commit history, staging changes, branching, and pushing/pulling. Think of it as a leaner, faster, more focused alternative to **GitKraken**.

It is built with Tauri + Vue, so it's a real native Mac app (small, fast, ~10MB) — not a heavy Electron wrapper. It should *feel* like a first-class Mac citizen.

## Who it's for

Professional software developers on macOS who live in Git all day. Primary user profile:
- Full-stack developer (Laravel / Vue background), comfortable in the terminal but wants a GUI for visual history, diffs, and staging.
- Works across **both GitHub and GitLab**, often with **multiple accounts** on each (e.g. a work GitLab, a personal GitLab, a self-managed company GitLab).
- Values speed, clarity, and keyboard-driven workflows. Dislikes bloat and subscription paywalls.

## Positioning — how it differs from GitKraken

Design should communicate these differentiators through its look and feel:
1. **Free, forever.** No account required, no paywalled features, no "Pro" upsell in the UI.
2. **Native & lightweight.** Fast, quiet, Mac-first. Not a resource hog.
3. **Best-in-class GitLab support** — GitLab is treated as an equal to GitHub, not an afterthought.
4. **True multi-account.** Multiple connections per provider (multiple GitLab/GitHub accounts and self-managed instances) are a core, visible concept — not a hidden setting.

## Brand personality

Pick a name and identity that feel: **precise, calm, fast, developer-trusted, modern, uncluttered.** Not playful-cartoonish, not enterprise-stuffy. Confident and tool-like — something a senior engineer would be happy to have in their dock.

## Naming brief

Propose 5–8 candidate names, then recommend one. Good directions to explore:
- Evocative of Git concepts (branches, graphs, trees, commits, flow, currents) without being literal or overused.
- Short, memorable, easy to say and type; ideally available as a `.app` name that isn't already a well-known dev tool.
- Avoid collisions with existing Git GUIs (GitKraken, Fork, Tower, Sourcetree, Sublime Merge, Lazygit, GitUp, Gitbutler, Gitify).
- Nautical/kraken clichés are the incumbent's territory — feel free to go a different direction.

For each candidate give: the name, a one-line rationale, and a quick note on tone. For the recommended name, sketch a **logo/mark direction** and a tagline.

## Visual direction (open — you decide)

Establish the full system. Some anchors, but you own the final call:
- **macOS-native feel:** respects the platform — vibrancy/translucency where tasteful, native-weight typography (SF Pro or similar), standard traffic-light window controls, comfortable density.
- **Light and dark themes**, dark as the hero (developers live in dark mode).
- A **commit-graph color language**: branch lanes need a distinct, accessible, colorblind-safe categorical palette that stays legible against both themes. This is the signature visual element — treat it as a centerpiece.
- Typography: a clean UI typeface plus a good **monospace** for diffs, hashes, and code.
- Restrained accent color; let the content (code, graph, diffs) carry the color.

## Core screens & components to design

The signature surface is the **repository workspace**. Please design:

1. **Main workspace layout**
   - Left sidebar: local/remote branches, tags, stashes, remotes, and — importantly — a **connected-accounts / integrations** section.
   - Center: the **commit graph** (DAG) with lanes, commit rows (avatar, message, author, hash, time), branch/tag labels.
   - Right / bottom panel: selected-commit detail and the **diff viewer**.
   - Top bar: repo switcher, current branch, fetch/pull/push actions, search.

2. **Commit graph** — the hero component. Show lane routing, merges, the current HEAD, branch tips, and how selection looks.

3. **Diff viewer** — side-by-side and inline; staged vs unstaged; hunk- and line-level staging controls; syntax highlighting.

4. **Staging / commit panel** — unstaged & staged lists, commit message box, amend, branch indicator.

5. **Accounts & integrations manager** — the multi-account centerpiece. A *list* of connections, each showing provider (GitHub / GitLab / self-managed), account label, avatar, and status. Adding a new connection (choose provider → instance URL for self-managed → authenticate). Make "I have three GitLab accounts and two GitHub accounts" feel natural and organized.

6. **PR / MR list** — pull requests (GitHub) and merge requests (GitLab) unified in one view, with status/pipeline/checks indicators.

7. **Command palette** — keyboard-first quick actions (⌘K style).

8. **Empty / onboarding state** — open a repo, clone, or connect an account. No forced sign-up.

9. **App icon** — a dock-worthy macOS app icon reflecting the chosen name.

## Constraints & notes

- Desktop only, single window with the workspace; secondary sheets/modals for clone, settings, and add-account flows.
- Keyboard-first; every primary action should feel reachable without the mouse.
- Accessibility: WCAG-AA contrast in both themes; the graph palette must be colorblind-safe.
- Deliver as a design system: color tokens, type scale, spacing, iconography style, and the annotated key screens above.

## What success looks like

A developer opens it, immediately understands the graph, stages a hunk, switches between their work and personal GitLab accounts without friction, and thinks: *"this is what GitKraken should have been — and it's free and it's fast."*
