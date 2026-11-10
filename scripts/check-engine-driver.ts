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

const tooling: CheckPhase[] = [
  { label: "tooling-tests", command: "node", args: ["--test", "scripts/check-engine-driver.test.ts", "scripts/capture-json-output.test.ts", "scripts/retain-policy-sources.test.ts", "scripts/retain-server-evidence.test.ts", "scripts/retention-workspace.test.ts", "scripts/run-selected-tests.test.ts"] },
  { label: "storage-safety", command: "python3", args: ["-B", "scripts/test-engine-storage.py"] },
  { label: "retention-safety", command: "python3", args: ["-B", "scripts/test-retention-workspace.py"] },
];
const archive: CheckPhase = { label: "archive-parser", command: "node", args: ["scripts/run-selected-tests.ts", "--name", "^retained archive hashes agree with JavaScript and source parser$", "packages/game-engine/tests/contracts.test.ts"], preflight: true };
const format: CheckPhase = { label: "format", command: "cargo", args: ["fmt", "--all", "--", "--check"] };

// Development coverage is deliberately smaller than historical acceptance.
// No default phase launches the simulator, release build, demo or profile matrix.
export const engineCheckPlans = {
  source: {
    phases: [...tooling, archive],
    coverage: "Tooling behavior/negatives and archived-source parser agreement; no Cargo.",
    omitted: "Rust formatting, compilation and tests; executable schema/catalog checks; runtime demo; SDK; release/profile/replay/retention gates.",
  },
  fast: {
    phases: [...tooling, archive, format,
      { label: "rust-check", command: "cargo", args: ["check", "--workspace", "--all-targets", "--locked"] },
      { label: "compiler-tests", command: "cargo", args: ["test", "--locked", "-p", "mtg-contracts", "-p", "mtg-compiler", "--lib"] }],
    coverage: "Source/tooling checks, Rust formatting, all-target compilation, contracts/compiler unit tests.",
    omitted: "Core/simulator/server runtime tests; Clippy; executable schema/catalog checks; runtime demo; SDK; release/profile/replay/retention gates.",
  },
  contracts: {
    phases: [...tooling, { label: "contract-preflight", command: "node", args: ["--test", "packages/game-engine/tests/contracts.test.ts"], preflight: true }],
    coverage: "Tooling and all source/schema/catalog Node contracts, including downgrade negatives; may build release mtg-sim.",
    omitted: "Rust formatting/lints/unit tests; runtime-backed Node contracts/demo; SDK; release test/profile/replay/retention gates.",
  },
  full: {
    phases: [format, ...tooling,
      { label: "clippy", command: "cargo", args: ["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"] },
      { label: "rust-tests", command: "cargo", args: ["test", "--workspace", "--locked"] },
      { label: "node-tests", command: "node", args: ["--test", "packages/game-engine/tests/contracts.test.ts"] }],
    coverage: "Former check:engine coverage: formatting, Clippy, workspace Rust tests and all tooling/Node contracts including demo. Cheap contracts run once.",
    omitted: "Ignored Rust diagnostics; SDK; separate release test/profile matrix/explicit CLI replay/retention gates. This is not universal acceptance.",
  },
} satisfies Record<string, { phases: CheckPhase[]; coverage: string; omitted: string }>;
export type CheckMode = keyof typeof engineCheckPlans;

export function parseCheckArguments(args: string[]) {
  let mode: CheckMode = "fast";
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
