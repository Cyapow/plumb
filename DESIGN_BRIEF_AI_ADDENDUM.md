# Design Brief — Addendum: Bring-Your-Own AI Agents

> Feed this to Claude Design as a follow-up to the Plumb design. It extends the commit composer (Plate 12) and AI grouping (Plate 13), and asks for one new plate: an **AI providers manager**. Keep everything in Plumb's existing system — Archivo/JetBrains Mono, `#ec3013`/`#ff563c` accent, 0px radius, 2px dividers, both themes, the same voice.

## The change in one line

The composer currently says *"Model: local · never leaves this Mac."* Replace that single hardcoded model with a **pluggable list of AI providers the user configures themselves** — mirroring how Plumb already treats Git accounts as a *list of connections*, not a single login. Local-and-private stays the **default**, so the privacy positioning is unchanged; cloud and custom agents are opt-in.

## Design principle: AI providers are connections too

Plumb's whole thesis is "multiple connections, done right." Apply the identical mental model to AI:

- AI providers are **a list**, each with `{type, label, endpoint/model, credential-ref, scope}`.
- The user can have several at once (e.g. a local Ollama model *and* a Claude API key *and* a custom CLI agent) and pick which is active.
- One is marked **default**; individual repos may override it.
- Credentials live in the **macOS Keychain** — never on disk, never sent to a Plumb server (there is no Plumb server).
- Every AI-produced artefact is **labelled and sourced**, and clearly flags **whether the data left the Mac**.

## Provider types to represent

Design the providers manager and the composer selector to accommodate these kinds. Show them as visually distinct rows/badges:

1. **Local** — Ollama / llama.cpp / an on-device model. Badge: private. The default. "Never leaves this Mac."
2. **Cloud API** — Anthropic (Claude), OpenAI, or any OpenAI-compatible endpoint via base URL + key. Badge: cloud. Must carry an explicit "sends your staged diff to `<host>`" indicator.
3. **Custom agent / command** — the user points Plumb at a CLI they already run (e.g. their own agent, `claude`, a shell script). Plumb pipes the diff/context in and reads the drafted message/grouping back out. Badge: custom. This is the "integrate your own AI agent" promise made literal.

(Design should not assume any of these is present — the empty state matters; see below.)

## New plate — AI providers manager (mirror the accounts manager, Plate 09)

A sheet titled something like **"AI providers."** A *list* of configured providers, each row showing:
- Type badge (Local / Cloud / Custom), colour-coded but never colour-only.
- User's label (e.g. "Ollama · qwen2.5-coder", "Claude · work key", "my-agent.sh").
- The model name or endpoint host, in JetBrains Mono.
- A **privacy indicator**: a filled square = stays local; an outlined/accent square = leaves the Mac, with the destination host named.
- **Default** marker (the same HEAD-style vocabulary the graph uses).
- Live status square (reachable / unreachable / needs key) — reuse the connection status-square pattern.

Plus:
- A **"+ Add provider"** flow: choose type → (Local: pick model / Cloud: base URL + key / Custom: command + how the diff is passed) → test → save. Same stepped-sheet feel as "add account."
- An **empty state**: "No AI providers yet — Plumb writes your own messages until you add one." AI is **additive, never required**; committing by hand is always the primary path.
- A one-line **trust footer** like the accounts sheet: *"Keys in your Keychain. No Plumb account. Local providers never touch the network."*

## Changes to the commit composer (Plate 12)

- Replace the static *"Model: local · never leaves this Mac"* line with a **provider selector**: the active provider's label + type badge + privacy indicator, clickable to switch (a small popover listing configured providers and "Add provider…"). When the active provider is cloud, the line reads plainly that the diff will be sent to the named host.
- Keep everything else that's already right: Generate is a **button, never automatic**; the AI DRAFT chip states what was read *and now also which provider produced it*; alternates; Shorter/More detail; conventional-commits toggle; the chip clears on first keystroke.
- If **no provider** is configured, the Generate button becomes a quiet "Set up AI…" that opens the providers sheet — the composer still fully works without it.

## Changes to AI grouping (Plate 13)

- The **AI GROUPING** chip gains the same provider attribution and privacy indicator as the composer.
- Grouping is likewise **opt-in and re-editable** — the current "drag a row to move it / Re-group / Back to one commit" affordances stay; AI proposes, the user disposes.

## Guardrails to express in the design

- **Privacy is a first-class, always-visible signal** — the user should never wonder whether a diff was sent off-device. Local is default and visually the "safe" state.
- **AI is never mandatory and never silent.** No provider = the app is a complete, hand-driven Git client. Nothing is generated without an explicit action.
- **Sourced & disposable.** Every draft says where it came from and is plain editable text the user owns.

## Deliverable

- One new plate: **AI providers manager** (list + add-provider stepped sheet + empty state + trust footer), in both themes.
- Revised **Plate 12** composer header showing the provider selector + privacy indicator.
- A small revision note on **Plate 13** for provider attribution.
