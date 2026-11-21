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
  preflight?: boolean;
}

// These verify the verifier against tiny fixtures/fake workers. Routine engine
// behavior checks retain the live guards below without rerunning these suites.
const harnessSelfTests: CheckPhase[] = [
  { label: "tooling-tests", command: "node", args: ["--test", "scripts/check-engine-driver.test.ts", "scripts/capture-json-output.test.ts", "scripts/retain-policy-sources.test.ts", "scripts/retain-server-evidence.test.ts", "scripts/retention-workspace.test.ts", "scripts/run-selected-tests.test.ts", "scripts/engine-environment.test.ts"] },
  { label: "storage-safety", command: "python3", args: ["-B", "scripts/test-engine-storage.py"] },
  { label: "retention-safety", command: "python3", args: ["-B", "scripts/test-retention-workspace.py"] },
  { label: "worktree-safety", command: "python3", args: ["-B", "scripts/test-engine-worktrees.py"] },
  { label: "structure-tests", command: "python3", args: ["-B", "scripts/test-engine-structure.py"] },
  { label: "cache-tests", command: "python3", args: ["-B", "scripts/test-engine-cache.py"] },
  { label: "test-binary-tests", command: "python3", args: ["-B", "scripts/test-engine-test-binaries.py"] },
  { label: "documentation-tests", command: "python3", args: ["-B", "scripts/test-check-docs.py"] },
];
const liveGuards: CheckPhase[] = [
  { label: "structure-guard", command: "python3", args: ["-B", "scripts/engine-structure-check.py"] },
  { label: "documentation-guard", command: "python3", args: ["-B", "scripts/check-docs.py"] },
];
export const cachePreflight: CheckPhase = { label: "cache-preflight", command: "python3", args: ["-B", "scripts/engine-cache.py", "check"] };

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

export function smokePhases(args: string[]): CheckPhase[] {
  const [mode, ...rest] = args;
  const valid = mode === "smoke" ? rest.length === 0 || (rest.length === 2 && rest[0] === "--artifact" && !!rest[1])
    : mode === "smoke-replay" && rest.length === 1 && !!rest[0];
  if (!valid) throw new Error("Usage: engine:smoke [-- --artifact NEW.json] or engine:smoke:replay -- FILE");
  return [cachePreflight, { label: mode!, command: "cargo", args: ["run", "--quiet", "--locked", "-p", "mtg-sim", "--", mode!, ...rest] }];
}
const archive: CheckPhase = { label: "archive-parser", command: "node", args: ["scripts/run-selected-tests.ts", "--name", "^retained archive hashes agree with JavaScript and source parser$", "packages/game-engine/tests/contracts.test.ts"], preflight: true };
const format: CheckPhase = { label: "format", command: "cargo", args: ["fmt", "--all", "--", "--check"] };
const omittedSelfTests = "Verifier harness self-tests (run --tooling or --full).";
// Unit tests only; `--tests` would also rerun every `--lib` unit binary.
const libTests: CheckPhase = { label: "lib-tests", command: "cargo", args: ["test", "--locked", "-p", "mtg-contracts", "-p", "mtg-compiler", "-p", "mtg-core", "--lib"] };
// Integration binaries only (`--test '*'`), built once and run concurrently:
// `cargo test` would run the shards of the former core monolith one by one.
const coreIntegration: CheckPhase = { label: "core-integration", command: "python3", args: ["-B", "scripts/engine-test-binaries.py", "--locked", "-p", "mtg-core", "-p", "mtg-witness", "--features", "mtg-core/testkit", "--test", "*"] };

// Development coverage is deliberately smaller than historical acceptance.
// No default phase launches the simulator, release build, demo or profile matrix.
export const engineCheckPlans = {
  quick: {
    needsCargo: true,
    phases: [cachePreflight, ...liveGuards, format,
      { label: "rust-lib-check", command: "cargo", args: ["check", "--workspace", "--lib", "--locked"] }],
    coverage: "Live structure/documentation guards and cache preflight, Rust formatting, workspace library compilation check.",
    omitted: `${omittedSelfTests} Archived-source parser agreement; compilation of binaries (including mtg-sim and mtg-server), tests and other non-library targets; all tests and Clippy; executable schema/catalog checks; runtime demo; SDK; release/profile/replay/retention gates.`,
  },
  source: {
    needsCargo: false,
    phases: [...liveGuards, archive],
    coverage: "Live structure/documentation guards and archived-source parser agreement; no Cargo or lock.",
    omitted: `${omittedSelfTests} Rust formatting, compilation and tests; executable schema/catalog checks; runtime demo; SDK; release/profile/replay/retention gates.`,
  },
  tooling: {
    needsCargo: false,
    phases: [...harnessSelfTests, ...liveGuards],
    coverage: "Complete verifier harness self-tests and live structure/documentation guards; no native setup, cache admission, Cargo or lock.",
    omitted: "Archived-source parser agreement; Rust formatting, compilation, lints and tests; executable schema/catalog checks; runtime demo; SDK; release/profile/replay/retention gates.",
  },
  fast: {
    needsCargo: true,
    phases: [cachePreflight, ...liveGuards, archive, format,
      { label: "rust-check", command: "cargo", args: ["check", "--workspace", "--all-targets", "--locked"] },
      libTests],
    coverage: "Live structure/documentation guards and cache preflight, archived-source parser agreement, Rust formatting, all-target compilation, contracts/compiler/core unit tests.",
    omitted: `${omittedSelfTests} Core/witness integration binaries (run --core); simulator/server runtime tests; Clippy; executable schema/catalog checks; runtime demo; SDK; release/profile/replay/retention gates.`,
  },
  core: {
    needsCargo: true,
    phases: [cachePreflight, ...liveGuards, libTests, coreIntegration],
    coverage: "Live guards/cache preflight, contracts/compiler/core unit tests, every mtg-core testkit scenario binary and every mtg-witness binary.",
    omitted: `${omittedSelfTests} Archived-source parser agreement; formatting; Clippy; simulator/server tests; executable schema/catalog checks; runtime demo; SDK; release/profile/replay/retention gates.`,
  },
  contracts: {
    needsCargo: true,
    phases: [cachePreflight, ...liveGuards, { label: "contract-preflight", command: "node", args: ["--test", "packages/game-engine/tests/contracts.test.ts"], preflight: true }],
    coverage: "Live structure/documentation guards and cache preflight, all source/schema/catalog Node contracts, including strict semantic/header negatives; may build release mtg-sim.",
    omitted: `${omittedSelfTests} Rust formatting/lints/unit tests; runtime-backed Node contracts/demo; SDK; release test/profile/replay/retention gates.`,
  },
  full: {
    needsCargo: true,
    phases: [cachePreflight, format, ...harnessSelfTests, ...liveGuards,
      { label: "clippy", command: "cargo", args: ["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"] },
      { label: "rust-tests", command: "cargo", args: ["test", "--workspace", "--locked"] },
      { label: "node-tests", command: "node", args: ["--test", "packages/game-engine/tests/contracts.test.ts"] }],
    coverage: "Live guards/cache preflight, complete verifier harness self-tests, formatting, Clippy, workspace Rust tests and all Node contracts including demo. Cheap contracts run once.",
    omitted: "Ignored Rust diagnostics; SDK; separate release test/profile matrix/explicit CLI replay/retention gates. This is not universal acceptance.",
  },
} satisfies Record<string, { needsCargo: boolean; phases: CheckPhase[]; coverage: string; omitted: string }>;
export type CheckMode = keyof typeof engineCheckPlans;

export function parseCheckArguments(args: string[]) {
  let mode: CheckMode = "quick";
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
      env: { ...(options.env ?? process.env), MTG_ENGINE_CONTRACT_PREFLIGHT: phase.preflight ? "1" : "0" },
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
