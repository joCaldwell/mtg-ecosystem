import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

type Result = { status: number | null; stdout: string };
type Check = { name: string; status: "ok" | "warn" | "fail"; detail: string; fix?: string };
type Options = {
  root: string;
  preflight?: boolean;
  env?: NodeJS.ProcessEnv;
  run?: (command: string, args: string[]) => Result;
  freeBytes?: (directory: string) => number;
};

export function inspectEngine(options: Options): Check[] {
  const { root, preflight = false, env = process.env } = options;
  const run = options.run ?? ((command, args) => {
    const result = spawnSync(command, args, {
      cwd: root, env: { ...env, RUSTUP_AUTO_INSTALL: "0" }, encoding: "utf8",
      timeout: 10_000, maxBuffer: 64 * 1024,
    });
    // Never print child stderr, environment values, npm config or remote URLs.
    return { status: result.status, stdout: result.stdout ?? "" };
  });
  const checks: Check[] = [];
  const add = (name: string, status: Check["status"], detail: string, fix?: string) => checks.push({ name, status, detail, ...(fix ? { fix } : {}) });
  const setup = "Use sh scripts/engine-env.sh COMMAND; see docs/game-engine-development.md#toolchain-setup. No toolchain is downloaded automatically.";
  const nodePin = fs.readFileSync(path.join(root, ".node-version"), "utf8").trim();
  const rustPin = fs.readFileSync(path.join(root, "rust-toolchain.toml"), "utf8").match(/^channel = "([^"]+)"/m)?.[1];
  if (!rustPin) throw new Error("Missing Rust channel pin in rust-toolchain.toml");
  for (const [command, expected] of [["node", nodePin], ["rustc", rustPin], ["cargo", rustPin]]) {
    const result = run(command, ["--version"]);
    const version = result.stdout.match(/(?:^v|^[a-z]+ )(\d+\.\d+\.\d+)(?:\s|$)/)?.[1];
    const good = result.status === 0 && version === expected;
    add(command, good ? "ok" : "fail", `required ${expected}; ${result.status === 0 && version ? `found ${version}` : "unavailable"}`, good ? undefined : setup);
  }
  for (const command of ["npm", "rustfmt", "cargo-clippy", "python3", "git", "flock"]) {
    const result = run(command, ["--version"]);
    const version = result.stdout.match(/\d+\.\d+(?:\.\d+)?/)?.[0];
    const good = result.status === 0 && !!version;
    add(command, good ? "ok" : "fail", good ? `available (${version})` : "unavailable", good ? undefined : setup);
  }
  if (process.platform !== "linux") add("platform", "fail", "Engine verification host tools require Linux.", "Use a Linux development host; no system changes were made.");

  if (preflight) add("dependencies", "ok", "Installation preflight: dependency check deferred until npm ci completes.");
  else {
    const require = createRequire(path.join(root, "package.json"));
    const lock = JSON.parse(fs.readFileSync(path.join(root, "package-lock.json"), "utf8"));
    for (const name of ["typescript", "@types/node", "ajv"]) {
      const expected = lock.packages[`node_modules/${name}`]?.version;
      let found: string | undefined;
      try { found = JSON.parse(fs.readFileSync(require.resolve(`${name}/package.json`), "utf8")).version; } catch { /* Actionable missing dependency below. */ }
      const good = !!expected && found === expected;
      add(`dependency ${name}`, good ? "ok" : "fail", `locked ${expected ?? "missing"}; found ${found ?? "missing"}`, good ? undefined : "Run sh scripts/engine-bootstrap.sh (do not run concurrently with another npm install).");
    }
    for (const name of ["game-engine", "oracle-parser"]) {
      let good = false;
      try { good = fs.realpathSync(path.join(root, "node_modules/@mtg-ecosystem", name)) === fs.realpathSync(path.join(root, "packages", name)); } catch { /* Missing workspace. */ }
      add(`workspace ${name}`, good ? "ok" : "fail", good ? "linked to this worktree" : "missing or points outside this worktree", good ? undefined : "Run sh scripts/engine-bootstrap.sh.");
    }
  }

  const freeBytes = options.freeBytes ?? ((directory: string) => {
    const stat = fs.statfsSync(directory);
    return stat.bavail * stat.bsize;
  });
  const target = path.resolve(root, env.CARGO_TARGET_DIR ?? "target");
  for (const [label, directory] of [["worktree disk", root], ["Cargo disk", target]]) {
    let existing = directory;
    while (!fs.existsSync(existing) && path.dirname(existing) !== existing) existing = path.dirname(existing);
    try {
      const available = freeBytes(existing);
      const gib = available / 1024 ** 3;
      const status = available < 256 * 1024 ** 2 ? "fail" : available < 2 * 1024 ** 3 ? "warn" : "ok";
      add(label, status, `${gib.toFixed(2)} GiB available; this is not a native-build budget approval`, status === "ok" ? undefined : "Inspect storage with the build owner; preserve evidence and existing caches. See docs/game-engine-storage.md.");
    } catch { add(label, "fail", "Cannot inspect filesystem capacity.", "Check directory access and mount availability."); }
  }
  add("Cargo cache", "ok", env.CARGO_TARGET_DIR ? "Explicit target inherited; no cache created or modified." : "Cargo default target; coordinate native builds and cache selection with the build owner.");
  const head = run("git", ["rev-parse", "--verify", "HEAD"]);
  if (head.status !== 0) add("Git", "fail", "Cannot resolve worktree HEAD.", "Run from a Git worktree with a committed base.");
  else {
    const branch = run("git", ["symbolic-ref", "--quiet", "HEAD"]);
    if (branch.status !== 0) add("branch", "warn", "Detached HEAD.", "Create a task branch before committing checkpoints.");
    const upstream = run("git", ["rev-parse", "--verify", "@{upstream}"]);
    if (upstream.status !== 0) add("upstream", "warn", "No locally resolvable upstream; no network fetch was attempted.", "After checking the destination, push with git push -u origin HEAD.");
    else {
      const counts = run("git", ["rev-list", "--left-right", "--count", "HEAD...@{upstream}"]);
      const match = counts.stdout.trim().match(/^(\d+)\s+(\d+)$/);
      if (counts.status !== 0 || !match) add("upstream", "warn", "Unable to compare local upstream.", "Inspect git status and tracking configuration.");
      else add("upstream", match[1] === "0" && match[2] === "0" ? "ok" : "warn", `ahead ${match[1]}, behind ${match[2]} relative to locally stored upstream; remote freshness unknown`, "Fetch explicitly when needed; inspect divergence before integrating. Never force-push shared work.");
    }
  }
  const sharedLock = path.join(os.homedir(), ".local/share/mtg-ecosystem/engine-verification/cargo.lock");
  add("native coordination", "ok", `Standalone Cargo commands require the shared advisory lock (${sharedLock}); check:engine owns its lock internally.`);
  return checks;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.some(arg => !["--preflight", "--json"].includes(arg))) {
    console.error("Usage: node scripts/engine-doctor.ts [--preflight] [--json]");
    process.exitCode = 2;
  } else {
    try {
      const checks = inspectEngine({ root: fileURLToPath(new URL("../", import.meta.url)), preflight: args.includes("--preflight") });
      if (args.includes("--json")) console.log(JSON.stringify({ checks, healthy: !checks.some(check => check.status === "fail") }, null, 2));
      else for (const check of checks) console.log(`${check.status.toUpperCase()} ${check.name}: ${check.detail}${check.fix ? `\n  ${check.fix}` : ""}`);
      process.exitCode = checks.some(check => check.status === "fail") ? 1 : 0;
    } catch {
      console.error("FAIL doctor could not read repository configuration. Check .node-version, rust-toolchain.toml and package-lock.json; child output and environment were withheld.");
      process.exitCode = 1;
    }
  }
}
