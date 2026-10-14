// Rebuild the retained native closure in a fresh dependency/target store.
// cargo vendor's returned config and --frozen behavior are documented at:
// https://doc.rust-lang.org/cargo/commands/cargo-vendor.html
// https://doc.rust-lang.org/cargo/reference/source-replacement.html
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const sha = (bytes: string | Buffer) => createHash("sha256").update(bytes).digest("hex");
function run(command: string, args: string[], cwd = root, env = process.env) {
  const result = spawnSync(command, args, { cwd, env, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.status !== 0) throw new Error(`${command} failed: ${result.status}\n${result.stdout}`);
  return result.stdout;
}
// Retain the exact official rules alongside sources; never refresh rules during replay.
const rulesPath = process.env.MTG_ENGINE_RULES_PATH ?? path.join(root, ".scryfall-cache/rules-20260925.txt");
const rulesBytes = fs.readFileSync(rulesPath);
const rulesSha256 = "8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca";
if (sha(rulesBytes) !== rulesSha256) throw new Error("Pinned official rules digest mismatch.");
const bundleDir = path.join(root, "generated/p2-bundles");
const report = JSON.parse(run("cargo", ["run", "--quiet", "--locked", "--release", "-p", "mtg-sim", "--", "demo", "--bundle", bundleDir]));
const matching = fs.readdirSync(bundleDir).filter(name => name.endsWith(".json")).map(name => path.join(bundleDir, name))
  .filter(file => JSON.parse(fs.readFileSync(file, "utf8")).build_id === report.build_id);
if (matching.length !== 1) throw new Error("Expected one exact retained release bundle.");
const bundleBytes = fs.readFileSync(matching[0]);
const bundle = JSON.parse(bundleBytes.toString("utf8"));
const storeRoot = process.argv[2] ? path.resolve(process.argv[2]) : path.join(os.homedir(), ".local/share/mtg-ecosystem/engine-evidence");
fs.mkdirSync(storeRoot, { recursive: true });
const store = fs.mkdtempSync(path.join(storeRoot, `p2-${sha(bundleBytes).slice(0, 16)}-`));
const retainedRulesPath = path.join(store, "rules-20260925.txt");
fs.writeFileSync(retainedRulesPath, rulesBytes);
const sourceRoot = path.join(store, "source");
fs.mkdirSync(sourceRoot);
for (const file of bundle.source_files) {
  if (path.isAbsolute(file.path) || file.path.split(/[\\/]/u).includes("..") || sha(file.contents) !== file.sha256) {
    throw new Error("Invalid retained source path/hash.");
  }
  const target = path.join(sourceRoot, file.path);
  fs.mkdirSync(path.dirname(target), { recursive: true });fs.writeFileSync(target, file.contents);
}
const bundlePath = path.join(store, `${sha(bundleBytes)}.json`);
fs.writeFileSync(bundlePath, bundleBytes);
const vendorPath = path.join(store, "vendor");
// Preparing the complete locked closure may fetch target-specific crates not
// used by this host build. Only the independent rebuild must be offline.
const config = run("cargo", ["vendor", "--locked", "--versioned-dirs", vendorPath]);
fs.mkdirSync(path.join(sourceRoot, ".cargo"));fs.writeFileSync(path.join(sourceRoot, ".cargo/config.toml"), config);

const dependencyFiles: Record<string, string> = {};
function recordFiles(dir: string) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0)) {
    const file = path.join(dir, entry.name);
    if (entry.isDirectory()) recordFiles(file);
    else dependencyFiles[path.relative(vendorPath, file)] = sha(fs.readFileSync(file));
  }
}
recordFiles(vendorPath);
const cargoHomePath = path.join(store, "fresh-cargo-home");fs.mkdirSync(cargoHomePath);
const targetPath = path.join(store, "fresh-target");
const env = { ...process.env, CARGO_HOME: cargoHomePath, CARGO_TARGET_DIR: targetPath,
  CARGO_NET_OFFLINE: "true", CARGO_ENCODED_RUSTFLAGS: bundle.rustflags,
  CARGO_PROFILE_RELEASE_OPT_LEVEL: bundle.opt_level,
  CARGO_PROFILE_RELEASE_DEBUG: bundle.debug,
  CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS: bundle.debug_assertions };
const initialCargoHomeEmpty = fs.readdirSync(cargoHomePath).length === 0;
run("cargo", ["build", "--frozen", "--release", "--target", bundle.target, "-p", "mtg-sim"], sourceRoot, env);
const rebuilt = path.join(targetPath, bundle.target, "release/mtg-sim");
const verification = JSON.parse(run(rebuilt, ["verify-bundle", bundlePath], store, env));
if (verification.state_sha256 !== report.state_sha256) throw new Error("Offline rebuilt replay differs.");
if (sha(fs.readFileSync(retainedRulesPath)) !== bundle.catalog.rules_sha256) throw new Error("Retained rules/catalog mismatch.");
const evidence = { rules_path: retainedRulesPath, rules_sha256: rulesSha256, build_id: report.build_id, catalog_digest: report.catalog_digest, profile_digest: report.profile_digest,
  state_sha256: report.state_sha256, bundle: bundlePath, bundle_sha256: sha(bundleBytes), rebuilt_executable: rebuilt,
  executable_sha256: sha(fs.readFileSync(rebuilt)), dependency_files: dependencyFiles, dependency_manifest_sha256: sha(JSON.stringify(dependencyFiles)),
  empty_cargo_home_before_build: initialCargoHomeEmpty, cargo_frozen_offline: true, verification,
  timing_observations: report.timing_observations, retained_branch_observations: report.retained_branch_observations };
fs.writeFileSync(path.join(store, "evidence.json"), JSON.stringify(evidence, null, 2) + "\n");
fs.mkdirSync(path.join(root, "generated"), { recursive: true });
fs.writeFileSync(path.join(root, "generated/p2-retention.json"), JSON.stringify(evidence, null, 2) + "\n");
console.log(JSON.stringify({ status: "offline_rebuild_and_replay_verified", store, build_id: report.build_id,
  state_sha256: verification.state_sha256, dependency_files: Object.keys(dependencyFiles).length }));
