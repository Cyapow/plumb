#!/usr/bin/env node
// Version-bump + release helper.
//
// Bumps the version across every manifest (tauri.conf.json is the source of
// truth, plus package.json, Cargo.toml, Cargo.lock), commits, tags plumb-vX.Y.Z,
// and pushes. The tag — not the push to main — is what triggers the Release CI.
//
// Usage:
//   node scripts/release.mjs <patch|minor|major|X.Y.Z> [--dry-run] [--yes]
//   npm run release -- minor
import { readFileSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const argv = process.argv.slice(2);
const bump = argv.find((a) => !a.startsWith("--"));
const dryRun = argv.includes("--dry-run");
const assumeYes = argv.includes("--yes");

if (!bump) {
  console.error("Usage: node scripts/release.mjs <patch|minor|major|X.Y.Z> [--dry-run] [--yes]");
  process.exit(1);
}

const p = {
  tauri: join(root, "src-tauri/tauri.conf.json"),
  pkg: join(root, "package.json"),
  cargo: join(root, "src-tauri/Cargo.toml"),
  lock: join(root, "src-tauri/Cargo.lock"),
};

const tauri = JSON.parse(readFileSync(p.tauri, "utf8"));
const current = tauri.version;
const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(current);
if (!m) {
  console.error(`Can't parse current version "${current}" in tauri.conf.json`);
  process.exit(1);
}
const [maj, min, pat] = m.slice(1).map(Number);

let next;
if (bump === "patch") next = `${maj}.${min}.${pat + 1}`;
else if (bump === "minor") next = `${maj}.${min + 1}.0`;
else if (bump === "major") next = `${maj + 1}.0.0`;
else if (/^\d+\.\d+\.\d+$/.test(bump)) next = bump;
else {
  console.error(`Invalid bump "${bump}" — use patch | minor | major | X.Y.Z`);
  process.exit(1);
}

console.log(`Plumb release: ${current} → ${next}  (tag plumb-v${next})`);

// Refuse to release on top of uncommitted changes so the release commit is clean.
const dirty = execSync("git status --porcelain", { cwd: root }).toString().trim();
if (dirty && !dryRun) {
  console.error("Working tree is not clean — commit or stash your changes first.");
  process.exit(1);
}

if (!assumeYes && !dryRun) {
  const rl = createInterface({ input: process.stdin, output: process.stdout });
  const ans = await new Promise((r) => rl.question(`Cut and push release plumb-v${next}? [y/N] `, r));
  rl.close();
  if (ans.trim().toLowerCase() !== "y") {
    console.log("Aborted.");
    process.exit(0);
  }
}

// --- edit manifests -------------------------------------------------------
tauri.version = next;
writeFileSync(p.tauri, JSON.stringify(tauri, null, 2) + "\n");

const pkg = JSON.parse(readFileSync(p.pkg, "utf8"));
pkg.version = next;
writeFileSync(p.pkg, JSON.stringify(pkg, null, 2) + "\n");

// First line-anchored `version = "..."` in Cargo.toml is the [package] version;
// dependency versions are inline (`x = { version = ... }`) so they're untouched.
writeFileSync(p.cargo, readFileSync(p.cargo, "utf8").replace(/^version = "[^"]+"/m, `version = "${next}"`));

// The plumb package block in the lockfile.
writeFileSync(
  p.lock,
  readFileSync(p.lock, "utf8").replace(/(name = "plumb"\nversion = ")[^"]+(")/, `$1${next}$2`),
);

if (dryRun) {
  console.log("Dry run — manifests updated, nothing committed/tagged/pushed.");
  process.exit(0);
}

// --- commit, tag, push ----------------------------------------------------
const tag = `plumb-v${next}`;
const run = (cmd) => execSync(cmd, { cwd: root, stdio: "inherit" });
run(`git add "${p.tauri}" "${p.pkg}" "${p.cargo}" "${p.lock}"`);
run(`git commit -m "chore(release): v${next}"`);
run(`git tag ${tag}`);
run(`git push --follow-tags`);
console.log(`\n✅ Released ${tag}. Watch the build: https://github.com/Cyapow/plumb/actions`);
