import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const wrapper = fileURLToPath(new URL("check-engine-command.py", import.meta.url));
export const digest = (bytes: string | Buffer) => createHash("sha256").update(bytes).digest("hex");

// This is a worktree inventory identity, not a claim that a binary contains all
// these files. Native executable build/source introspection remains required.
export function sourceIdentity(root: string) {
  const git = (...args: string[]) => {
    const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
    if (result.status !== 0) throw new Error(`git ${args[0]} failed`);
    return result.stdout;
  };
  const names = [...new Set(git("ls-files", "-z", "--cached", "--others", "--exclude-standard").split("\0").filter(Boolean))].sort();
  const inventory = names.map(name => {
    const filename = path.join(root, name);
    if (!fs.existsSync(filename)) return [name, "missing"];
    const stat = fs.lstatSync(filename);
    return [name, stat.isSymbolicLink() ? "symlink" : "file", stat.mode & 0o777, digest(stat.isSymbolicLink() ? fs.readlinkSync(filename) : fs.readFileSync(filename))];
  });
  return { git_head: git("rev-parse", "HEAD").trim(), git_tree: git("rev-parse", "HEAD^{tree}").trim(), worktree_inventory_sha256: digest(JSON.stringify(inventory)), files: names.length };
}

function executableIdentity(command: string, options: { cwd: string; env?: NodeJS.ProcessEnv }) {
  const candidates = command.includes(path.sep)
    ? [path.resolve(options.cwd, command)]
    : ((options.env ?? process.env).PATH ?? "").split(path.delimiter).map(directory => path.resolve(options.cwd, directory, command));
  for (const candidate of candidates) {
    try {
      fs.accessSync(candidate, fs.constants.X_OK);
      const filename = fs.realpathSync(candidate);
      // Multicall proxies (including rustup's cargo symlink) select behavior
      // using the invoked basename. Hash the target without invoking its name.
      return { path: candidate, resolved_path: filename, sha256: digest(fs.readFileSync(filename)) };
    } catch { /* Try the next PATH entry. */ }
  }
  throw new Error(`Executable not found: ${command}`);
}

// Dedicated groups allow interruption to reach Cargo, test binaries and their
// descendants. No detached execution survives a normally handled cancellation.
async function child(command: string, args: string[], options: { cwd: string; env?: NodeJS.ProcessEnv; lockFd?: number }) {
  const processChild = spawn(command, args, {
    cwd: options.cwd, env: options.env ?? process.env, detached: true,
    stdio: options.lockFd === undefined ? "inherit" : ["inherit", "inherit", "inherit", options.lockFd],
  });
  const handlers = new Map<NodeJS.Signals, () => void>();
  let cancelledSignal: NodeJS.Signals | undefined;
  let escalation: NodeJS.Timeout | undefined;
  const killGroup = (signal: NodeJS.Signals) => {
    if (processChild.pid) {
      try { process.kill(-processChild.pid, signal); } catch (error: any) { if (error.code !== "ESRCH") throw error; }
    }
  };
  for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"] as const) {
    const handler = () => {
      cancelledSignal = signal;
      killGroup(signal);
      // Bound cancellation even when a command ignores graceful termination.
      escalation ??= setTimeout(() => killGroup("SIGKILL"), 5000);
    };
    handlers.set(signal, handler); process.on(signal, handler);
  }
  try {
    return await new Promise<{ code: number | null; signal: NodeJS.Signals | null }>((resolve, reject) => {
      processChild.once("error", reject);
      processChild.once("close", (code, signal) => resolve({ code: cancelledSignal ? 128 + os.constants.signals[cancelledSignal] : code, signal: cancelledSignal ?? signal }));
    });
  } finally {
    clearTimeout(escalation);
    if (cancelledSignal) killGroup("SIGKILL");
    for (const [signal, handler] of handlers) process.off(signal, handler);
  }
}

export interface MeasurementOptions {
  cwd: string;
  env?: NodeJS.ProcessEnv;
  record: (value: any) => void;
}

export interface CheckPhase {
  label: string;
  command: string;
  args: string[];
}

// These verify the verifier against tiny fixtures/fake workers. Routine engine
// behavior checks retain the live guards below without rerunning these suites.
const harnessSelfTests: CheckPhase[] = [
  { label: "tooling-tests", command: "node", args: ["--test", "scripts/check-engine-driver.test.ts", "scripts/engine-environment.test.ts"] },
  { label: "storage-safety", command: "python3", args: ["-B", "scripts/test-engine-storage.py"] },
  { label: "worktree-safety", command: "python3", args: ["-B", "scripts/test-engine-worktrees.py"] },
  { label: "structure-tests", command: "python3", args: ["-B", "scripts/test-engine-structure.py"] },
  { label: "cache-tests", command: "python3", args: ["-B", "scripts/test-engine-cache.py"] },
  { label: "test-binary-tests", command: "python3", args: ["-B", "scripts/test-engine-test-binaries.py"] },
  { label: "deck-harness-tests", command: "python3", args: ["-B", "scripts/test-engine-decks.py"] },
  { label: "stage-deck-tests", command: "python3", args: ["-B", "scripts/test-stage-deck.py"] },
  { label: "documentation-tests", command: "python3", args: ["-B", "scripts/test-check-docs.py"] },
];
const liveGuards: CheckPhase[] = [
  { label: "structure-guard", command: "python3", args: ["-B", "scripts/engine-structure-check.py"] },
  { label: "documentation-guard", command: "python3", args: ["-B", "scripts/check-docs.py"] },
  // The compiler's typed archive AST is generated from the parser's TypeScript.
  { label: "generated-ast-guard", command: "node", args: ["scripts/generate-rust-ast.ts", "--check"] },
];
// Runs under the target lock this driver holds, so it may evict an over-budget target.
export const cachePreflight: CheckPhase = { label: "cache-preflight", command: "python3", args: ["-B", "scripts/engine-cache.py", "check", "--locked"] };

// Each worktree selects its own target (explicit CARGO_TARGET_DIR wins). Cargo
// profiles own incrementality: dev/test/quick are incremental and release is
// not, so CARGO_INCREMENTAL stays unset unless the caller chose a value.
export function nativeEnvironment(root: string, base: NodeJS.ProcessEnv = process.env): NodeJS.ProcessEnv {
  const selected = spawnSync("python3", ["-B", "scripts/engine-cache.py", "path"], { cwd: root, env: base, encoding: "utf8" });
  if (selected.status !== 0) throw new Error("Cannot select Cargo target; run engine:doctor.");
  return { ...base, CARGO_TARGET_DIR: selected.stdout.trim() };
}

// The advisory lock of the selected target. Worktrees with distinct targets never
// wait on each other; commands sharing one target serialize.
export function nativeLock(root: string, env: NodeJS.ProcessEnv) {
  const selected = spawnSync("python3", ["-B", "scripts/engine-cache.py", "lock"], { cwd: root, env, encoding: "utf8" });
  if (selected.status !== 0 || !path.isAbsolute(selected.stdout.trim())) throw new Error("Cannot select the Cargo target lock; run engine:doctor.");
  return selected.stdout.trim();
}

export function pinnedRust(root: string) {
  const pin = fs.readFileSync(path.join(root, "rust-toolchain.toml"), "utf8").match(/^channel\s*=\s*"([^"]+)"/m)?.[1];
  if (!pin) throw new Error("Missing Rust channel in rust-toolchain.toml");
  const version = spawnSync("rustc", ["--version"], { encoding: "utf8" });
  if (version.status !== 0 || !version.stdout.startsWith(`rustc ${pin} `)) throw new Error(`Engine checks require pinned Rust ${pin}. Use sh scripts/engine-env.sh COMMAND.`);
  return version.stdout.trim();
}

// One feature shape for every debug Cargo command, so check, Clippy and tests
// share one compiled engine. A separate metadata-only check of the host without
// `testkit` keeps dead-code warnings visible to the production feature set.
const SHAPE = ["--locked", "--workspace", "--features", "mtg-core/testkit"];
const format: CheckPhase = { label: "format", command: "cargo", args: ["fmt", "--all", "--", "--check"] };
const productionCheck: CheckPhase = { label: "production-check", command: "cargo", args: ["check", "--locked", "-p", "mtg-server", "-p", "mtg-sim"] };
const archiveAgreement: CheckPhase = { label: "archive-parser", command: "node", args: ["--test", "packages/oracle-parser/test/engine-archive-agreement.test.ts"] };

export const engineCheckPlans = {
  check: {
    needsCargo: true,
    phases: [cachePreflight, ...liveGuards, format,
      { label: "rust-check", command: "cargo", args: ["check", ...SHAPE, "--all-targets"] },
      productionCheck],
    coverage: "Live structure/documentation/generated-AST guards and cache admission, Rust formatting, all-target workspace compilation (testkit shape), production host/simulator compilation.",
    omitted: "Clippy; all tests; parser archive agreement; verifier harness self-tests (run check:engine:gate).",
  },
  gate: {
    needsCargo: true,
    phases: [cachePreflight, ...liveGuards, archiveAgreement, format,
      { label: "clippy", command: "cargo", args: ["clippy", ...SHAPE, "--all-targets", "--", "-D", "warnings"] },
      productionCheck,
      // Unit tests of every library and binary (`--tests` would rerun them).
      { label: "unit-tests", command: "cargo", args: ["test", ...SHAPE, "--lib", "--bins"] },
      { label: "doc-tests", command: "cargo", args: ["test", ...SHAPE, "--doc"] },
      // Core/witness integration binaries, built once and run concurrently.
      { label: "core-integration", command: "python3", args: ["-B", "scripts/engine-test-binaries.py", "--locked", "-p", "mtg-core", "-p", "mtg-witness", "--features", "mtg-core/testkit", "--test", "*"] },
      { label: "host-integration", command: "python3", args: ["-B", "scripts/engine-test-binaries.py", "--locked", "-p", "mtg-server", "--features", "mtg-core/testkit", "--test", "*"] },
      ...harnessSelfTests],
    coverage: "Everything in check:engine plus parser archive agreement, Clippy with warnings denied, every workspace unit, doc and integration test, and all verifier harness self-tests.",
    omitted: "Opt-in host witnesses that need external services; whole-archive coverage; release builds; deck benches (engine:decks).",
  },
  tooling: {
    needsCargo: false,
    phases: [...harnessSelfTests, ...liveGuards],
    coverage: "Verifier harness self-tests and live structure/documentation guards; no Cargo, cache admission or lock.",
    omitted: "Rust formatting, compilation, Clippy and tests; parser archive agreement.",
  },
} satisfies Record<string, { needsCargo: boolean; phases: CheckPhase[]; coverage: string; omitted: string }>;
export type CheckMode = keyof typeof engineCheckPlans;

export function parseCheckArguments(args: string[]) {
  let mode: CheckMode = "check";
  let selected = false;
  let planOnly = false;
  let help = false;
  for (const arg of args) {
    if (arg === "--plan") planOnly = true;
    else if (arg === "--help") help = true;
    else if (arg.startsWith("--") && Object.hasOwn(engineCheckPlans, arg.slice(2))) {
      if (selected) throw new Error("Select exactly one engine check mode.");
      mode = arg.slice(2) as CheckMode;
      selected = true;
    } else throw new Error(`Unknown engine check argument: ${arg}`);
  }
  return { mode, planOnly, help };
}

export async function runCheckPhases(phases: readonly CheckPhase[], options: MeasurementOptions, execute = measuredCommand) {
  const completed: string[] = [];
  for (const phase of phases) {
    const result = await execute(phase.label, phase.command, [...phase.args], {
      ...options,
      env: options.env ?? process.env,
    });
    if (result.code !== 0) return { code: result.code ?? 1, completed, failed: phase.label };
    completed.push(phase.label);
  }
  return { code: 0, completed, failed: null };
}

export async function measuredCommand(label: string, command: string, args: string[], options: MeasurementOptions) {
  const identity = executableIdentity(command, options);
  const python = executableIdentity("python3", options);
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "mtg-engine-time-"));
  const output = path.join(directory, "usage.json");
  const started = performance.now();
  const drivers = [wrapper, fileURLToPath(import.meta.url), fileURLToPath(new URL("check-engine.ts", import.meta.url))].map(filename => ({ path: filename, sha256: digest(fs.readFileSync(filename)) }));
  const host = { load_average: os.loadavg(), free_memory_bytes: os.freemem(), logical_cpus: os.availableParallelism() };
  let result: Awaited<ReturnType<typeof child>> | undefined;
  let failure: unknown;
  try {
    result = await child(python.path, [wrapper, output, identity.path, ...args], options);
    return result;
  } catch (error) { failure = error; throw error; }
  finally {
    try {
      options.record({ kind: "command", label, command: identity.path, args, executable: identity, measurement_drivers: drivers, python, cwd: options.cwd, started_at: new Date(Date.now() - (performance.now() - started)).toISOString(), dispatch_wall_seconds: (performance.now() - started) / 1000, host, driver_command_concurrency: 1, internal_concurrency: "command_default", other_host_concurrency: "not_measured", ...result, error: failure instanceof Error ? failure.message : undefined, usage: fs.existsSync(output) ? JSON.parse(fs.readFileSync(output, "utf8")) : null });
    } finally { fs.rmSync(directory, { recursive: true, force: true }); }
  }
}

export async function withAdvisoryLock<T>(filename: string, options: MeasurementOptions, work: () => Promise<T>): Promise<T> {
  fs.mkdirSync(path.dirname(filename), { recursive: true });
  const descriptor = fs.openSync(filename, "a");
  const started = performance.now();
  try {
    // flock and Node share one open file description. The lock remains held by
    // this descriptor after flock exits, without locking nested Cargo again.
    const result = await child("flock", ["--exclusive", "3"], { cwd: options.cwd, lockFd: descriptor });
    options.record({ kind: "lock", path: filename, wait_wall_seconds: (performance.now() - started) / 1000, ...result });
    if (result.code !== 0) throw new Error(`Lock acquisition interrupted: ${result.signal ?? result.code}`);
    return await work();
  } finally { fs.closeSync(descriptor); }
}
