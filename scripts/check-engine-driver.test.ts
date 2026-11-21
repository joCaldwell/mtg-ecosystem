import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { engineCheckPlans, parseCheckArguments, runCheckPhases, measuredCommand, nativeEnvironment, nativeLock, sourceIdentity, withAdvisoryLock, smokePhases } from "./check-engine-driver.ts";

test("default coverage is bounded and deeper runtime work is explicit", () => {
  const { mode } = parseCheckArguments([]);
  assert.equal(mode, "quick");
  const quick = engineCheckPlans[mode];
  assert.deepEqual(quick.phases.filter(phase => phase.command === "cargo").map(phase => phase.args), [
    ["fmt", "--all", "--", "--check"],
    ["check", "--workspace", "--lib", "--locked"],
  ]);
  assert.match(quick.omitted, /compilation of binaries \(including mtg-sim and mtg-server\)/);
  const fast = engineCheckPlans.fast.phases;
  assert.deepEqual(fast.find(phase => phase.label === "lib-tests")?.args, ["test", "--locked", "-p", "mtg-contracts", "-p", "mtg-compiler", "-p", "mtg-core", "--lib"]);
  const core = engineCheckPlans.core.phases;
  assert.deepEqual(core.find(phase => phase.label === "core-integration")?.args, ["-B", "scripts/engine-test-binaries.py", "--locked", "-p", "mtg-core", "-p", "mtg-witness", "--features", "mtg-core/testkit", "--test", "*"]);
  assert.deepEqual(core.find(phase => phase.label === "lib-tests"), fast.find(phase => phase.label === "lib-tests"));
  // `--tests` after `--lib` reruns every unit binary; integration binaries use `--test '*'`.
  for (const plan of Object.values(engineCheckPlans)) assert.ok(!plan.phases.some(phase => phase.args.includes("--tests")));
  assert.deepEqual(fast.find(phase => phase.label === "rust-check")?.args, ["check", "--workspace", "--all-targets", "--locked"]);
  assert.ok(fast.some(phase => phase.label === "archive-parser"));
  assert.ok(!fast.some(phase => ["clippy", "rust-tests", "node-tests", "contract-preflight"].includes(phase.label)));
  assert.ok(!fast.some(phase => phase.args.includes("--release")));
  assert.equal(engineCheckPlans.source.phases.some(phase => phase.command === "cargo"), false);
  const full = engineCheckPlans.full.phases;
  assert.deepEqual(full.find(phase => phase.label === "rust-tests")?.args, ["test", "--workspace", "--locked"]);
  assert.equal(full.filter(phase => phase.args.includes("packages/game-engine/tests/contracts.test.ts")).length, 1);
  assert.equal("preflight" in full.find(phase => phase.label === "node-tests")!, false);
  assert.match(engineCheckPlans.fast.omitted, /Core\/witness integration binaries \(run --core\); simulator\/server runtime tests/);
});

test("routine plans retain live guards and behavior; tooling/full retain every harness suite", () => {
  const suites = [
    ["tooling-tests", "node", ["--test", "scripts/check-engine-driver.test.ts", "scripts/capture-json-output.test.ts", "scripts/retain-policy-sources.test.ts", "scripts/retain-server-evidence.test.ts", "scripts/retention-workspace.test.ts", "scripts/run-selected-tests.test.ts", "scripts/engine-environment.test.ts"]],
    ["storage-safety", "python3", ["-B", "scripts/test-engine-storage.py"]],
    ["retention-safety", "python3", ["-B", "scripts/test-retention-workspace.py"]],
    ["worktree-safety", "python3", ["-B", "scripts/test-engine-worktrees.py"]],
    ["structure-tests", "python3", ["-B", "scripts/test-engine-structure.py"]],
    ["cache-tests", "python3", ["-B", "scripts/test-engine-cache.py"]],
    ["test-binary-tests", "python3", ["-B", "scripts/test-engine-test-binaries.py"]],
    ["documentation-tests", "python3", ["-B", "scripts/test-check-docs.py"]],
  ];
  for (const [mode, plan] of Object.entries(engineCheckPlans)) {
    const native = ["quick", "fast", "core", "contracts", "full"].includes(mode);
    const selfTests = ["tooling", "full"].includes(mode);
    assert.equal(plan.needsCargo, native);
    assert.equal(plan.phases.filter(phase => phase.label === "cache-preflight").length, native ? 1 : 0);
    if (native) assert.deepEqual(plan.phases[0], { label: "cache-preflight", command: "python3", args: ["-B", "scripts/engine-cache.py", "check"] });
    assert.equal(plan.phases.filter(phase => phase.args.includes("scripts/engine-structure-check.py")).length, 1);
    assert.equal(plan.phases.filter(phase => phase.args.includes("scripts/check-docs.py")).length, 1);
    assert.deepEqual(plan.phases.filter(phase => suites.some(([label]) => label === phase.label)).map(phase => [phase.label, phase.command, phase.args]), selfTests ? suites : []);
    if (!selfTests) assert.match(plan.omitted, /Verifier harness self-tests \(run --tooling or --full\)/);
  }
  const labels = (mode: keyof typeof engineCheckPlans) => engineCheckPlans[mode].phases.map(phase => phase.label);
  assert.deepEqual(labels("quick"), ["cache-preflight", "structure-guard", "documentation-guard", "generated-ast-guard", "format", "rust-lib-check"]);
  assert.deepEqual(labels("source"), ["structure-guard", "documentation-guard", "generated-ast-guard", "archive-parser"]);
  assert.deepEqual(labels("fast"), ["cache-preflight", "structure-guard", "documentation-guard", "generated-ast-guard", "archive-parser", "format", "rust-check", "lib-tests"]);
  assert.deepEqual(labels("core"), ["cache-preflight", "structure-guard", "documentation-guard", "generated-ast-guard", "lib-tests", "core-integration"]);
  assert.deepEqual(labels("contracts"), ["cache-preflight", "structure-guard", "documentation-guard", "generated-ast-guard", "contract-preflight"]);
  assert.deepEqual(labels("tooling"), [...suites.map(([label]) => label), "structure-guard", "documentation-guard", "generated-ast-guard"]);
  assert.deepEqual(labels("full"), ["cache-preflight", "format", ...suites.map(([label]) => label), "structure-guard", "documentation-guard", "generated-ast-guard", "clippy", "rust-tests", "node-tests"]);
  assert.deepEqual(engineCheckPlans.source.phases.at(-1), engineCheckPlans.fast.phases.find(phase => phase.label === "archive-parser"));
  assert.equal(engineCheckPlans.contracts.phases.at(-1)?.preflight, true);
  assert.equal(engineCheckPlans.source.phases.at(-1)?.preflight, true);
  const scripts = JSON.parse(fs.readFileSync(new URL("../package.json", import.meta.url), "utf8")).scripts;
  assert.equal(scripts["check:engine"], "sh scripts/engine-env.sh node scripts/check-engine.ts");
  assert.equal(scripts["check:engine:fast"], "sh scripts/engine-env.sh node scripts/check-engine.ts --fast");
  assert.equal(scripts["check:engine:core"], "sh scripts/engine-env.sh node scripts/check-engine.ts --core");
  assert.equal(scripts["check:engine:tooling"], "sh scripts/engine-env.sh node scripts/check-engine.ts --tooling");
});

// Run the real CLI/driver/measurement wrapper inside a tiny Git repository.
// Phase executables record dispatch and reject native setup, admission or locks.
// This avoids recursive tooling tests while exercising the CLI's execution path.
function withCheckCli(work: (fixture: {
  root: string;
  home: string;
  run: (args: string[], extraEnv?: NodeJS.ProcessEnv) => {
    status: number | null; stdout: string; stderr: string; calls: any[]; records: any[];
  };
}) => void) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "mtg-engine-cli-test-"));
  const root = path.join(directory, "repo");
  const home = path.join(directory, "home");
  const bin = path.join(directory, "bin");
  const trace = path.join(directory, "calls.jsonl");
  const python = spawnSync("python3", ["-c", "import sys; print(sys.executable)"], { encoding: "utf8" });
  assert.equal(python.status, 0, python.stderr);
  let sequence = 0;
  try {
    fs.mkdirSync(path.join(root, "scripts"), { recursive: true });
    fs.mkdirSync(bin);
    for (const name of ["check-engine.ts", "check-engine-driver.ts", "check-engine-command.py"]) {
      fs.copyFileSync(fileURLToPath(new URL(name, import.meta.url)), path.join(root, "scripts", name));
    }
    fs.writeFileSync(path.join(root, "rust-toolchain.toml"), 'channel = "1.99.0"\n');
    const proxy = `#!${process.execPath}\n` + String.raw`
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const command = path.basename(process.argv[1]);
const args = process.argv.slice(2);
if (command === "python3" && args[0].endsWith("/check-engine-command.py")) {
  const result = spawnSync(process.env.TEST_REAL_PYTHON, args, { stdio: "inherit" });
  process.exit(result.status ?? 99);
}
fs.appendFileSync(process.env.TEST_TRACE, JSON.stringify({command, args}) + "\n");
if (["rustc", "cargo", "flock"].includes(command) || args.includes("scripts/engine-cache.py")) process.exit(91);
if (args.includes(process.env.TEST_MUTATE_PHASE)) fs.writeFileSync("mutated-source", "changed");
process.exit(args.includes(process.env.TEST_FAIL_PHASE) ? 7 : 0);
`;
    for (const name of ["node", "python3", "rustc", "cargo", "flock"]) fs.writeFileSync(path.join(bin, name), proxy, { mode: 0o755 });
    for (const args of [["init", "--quiet"], ["add", "."], ["-c", "commit.gpgsign=false", "-c", "user.name=Focused test", "-c", "user.email=focused@example.invalid", "commit", "--quiet", "-m", "fixture"]]) {
      const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
      assert.equal(result.status, 0, result.stderr);
    }
    work({ root, home, run: (args, extraEnv = {}) => {
      const report = path.join(directory, `timing-${sequence++}.jsonl`);
      fs.writeFileSync(trace, "");
      const result = spawnSync(process.execPath, [path.join(root, "scripts/check-engine.ts"), ...args], {
        cwd: root, encoding: "utf8", timeout: 10000,
        env: { ...process.env, HOME: home, PATH: `${bin}${path.delimiter}${process.env.PATH}`, MTG_ENGINE_TIMING_FILE: report, TEST_TRACE: trace, TEST_REAL_PYTHON: python.stdout.trim(), ...extraEnv },
      });
      assert.ifError(result.error);
      const jsonLines = (filename: string) => fs.readFileSync(filename, "utf8").split("\n").filter(Boolean).map(line => JSON.parse(line));
      return { ...result, calls: jsonLines(trace), records: fs.existsSync(report) ? jsonLines(report) : [] };
    } });
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
}

test("tooling CLI dispatches complete self-tests and guards without native setup or lock", () => {
  withCheckCli(({ home, run }) => {
    const result = run(["--tooling"]);
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(result.calls, engineCheckPlans.tooling.phases.map(({ command, args }) => ({ command, args })));
    assert.deepEqual(result.records.filter(record => record.kind === "command").map(record => record.label), engineCheckPlans.tooling.phases.map(phase => phase.label));
    assert.equal(result.records.some(record => record.kind === "lock"), false);
    assert.equal(fs.existsSync(home), false, "no native lock directory should be created");
    assert.equal(result.records[0].rust, null);
    assert.equal(result.records.at(-1).unchanged, true);
    assert.match(result.stderr, /Omitted: Archived-source parser agreement/);
  });
});

test("CLI plan/help execute no commands or native setup for any mode", () => {
  withCheckCli(({ home, run }) => {
    for (const [mode, plan] of Object.entries(engineCheckPlans)) {
      const result = run([`--${mode}`, "--plan"]);
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(JSON.parse(result.stdout), { mode, ...plan });
      assert.deepEqual(result.calls, []);
      assert.deepEqual(result.records, []);
    }
    const implicit = run(["--plan"]);
    assert.equal(implicit.status, 0, implicit.stderr);
    assert.deepEqual(JSON.parse(implicit.stdout), { mode: "quick", ...engineCheckPlans.quick });
    assert.deepEqual(implicit.calls, []);
    assert.deepEqual(implicit.records, []);
    const help = run(["--tooling", "--help"]);
    assert.equal(help.status, 0, help.stderr);
    assert.match(help.stdout, /Default: --quick/);
    assert.match(help.stdout, /--tooling/);
    assert.deepEqual(help.calls, []);
    assert.deepEqual(help.records, []);
    assert.equal(fs.existsSync(home), false);
  });
});

test("tooling CLI propagates suite/guard failures and rejects changed source", () => {
  withCheckCli(({ run }) => {
    for (const name of ["scripts/test-engine-cache.py", "scripts/engine-structure-check.py", "scripts/check-docs.py"]) {
      const result = run(["--tooling"], { TEST_FAIL_PHASE: name });
      const failedIndex = engineCheckPlans.tooling.phases.findIndex(phase => phase.args.includes(name));
      assert.equal(result.status, 7, result.stderr);
      assert.deepEqual(result.calls, engineCheckPlans.tooling.phases.slice(0, failedIndex + 1).map(({ command, args }) => ({ command, args })));
      assert.equal(result.records.find(record => record.kind === "phases-complete").failed, engineCheckPlans.tooling.phases[failedIndex].label);
      assert.equal(result.records.at(-1).unchanged, true);
    }
    const changed = run(["--tooling"], { TEST_MUTATE_PHASE: "scripts/check-docs.py" });
    assert.equal(changed.status, 1, changed.stderr);
    assert.equal(changed.records.at(-1).unchanged, false);
    assert.match(changed.stderr, /source changed during execution/);
  });
});

test("source CLI runs live guards and the exact archive selector without native setup", () => {
  withCheckCli(({ home, run }) => {
    const result = run(["--source"]);
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(result.calls, engineCheckPlans.source.phases.map(({ command, args }) => ({ command, args })));
    assert.equal(fs.existsSync(home), false);
    assert.match(result.stderr, /Omitted: Verifier harness self-tests/);
    assert.equal(result.records.at(-1).unchanged, true);
  });
});

test("mode selection rejects typos and conflicting modes before launching commands", () => {
  assert.deepEqual(parseCheckArguments(["--contracts", "--plan"]), { mode: "contracts", planOnly: true, help: false });
  assert.deepEqual(parseCheckArguments(["--tooling"]), { mode: "tooling", planOnly: false, help: false });
  assert.throws(() => parseCheckArguments(["--ful"]), /Unknown/);
  assert.throws(() => parseCheckArguments(["--full", "--fast"]), /exactly one/);
  assert.throws(() => parseCheckArguments(["--full", "--full"]), /exactly one/);
  assert.throws(() => parseCheckArguments(["--tooling", "--source"]), /exactly one/);
});

test("phase failure stops subsequent work and inherited preflight cannot shrink full coverage", async () => {
  for (const code of [7, 143, null]) {
    const calls: any[] = [];
    const result = await runCheckPhases(engineCheckPlans.full.phases, {
      cwd: process.cwd(), record: () => {}, env: { MTG_ENGINE_CONTRACT_PREFLIGHT: "1", CARGO_INCREMENTAL: "0" },
    }, async (label, command, args, options) => {
      calls.push({ label, env: options.env });
      return { code, signal: null };
    });
    assert.equal(calls.length, 1);
    assert.equal(calls[0].env.MTG_ENGINE_CONTRACT_PREFLIGHT, "0");
    assert.equal(calls[0].env.CARGO_INCREMENTAL, "0");
    assert.deepEqual(result, { code: code ?? 1, completed: [], failed: "cache-preflight" });
  }
  const cohorts: string[] = [];
  const result = await runCheckPhases(engineCheckPlans.contracts.phases, {
    cwd: process.cwd(), record: () => {}, env: { MTG_ENGINE_CONTRACT_PREFLIGHT: "0" },
  }, async (label, command, args, options) => {
    cohorts.push(options.env!.MTG_ENGINE_CONTRACT_PREFLIGHT!);
    return { code: 0, signal: null };
  });
  assert.deepEqual(cohorts, engineCheckPlans.contracts.phases.map(phase => "preflight" in phase && phase.preflight ? "1" : "0"));
  assert.deepEqual(result, { code: 0, completed: engineCheckPlans.contracts.phases.map(phase => phase.label), failed: null });
});

test("smoke arguments preserve file boundaries and reject misuse before native work", () => {
  for (const args of [["smoke"], ["smoke", "--artifact", "a b.json"], ["smoke-replay", "a b.json"]]) {
    const phases = smokePhases(args);
    assert.equal(phases[0].label, "cache-preflight");
    assert.deepEqual(phases[1].args.slice(6), args);
  }
  for (const args of [[], ["smoke", "--unknown"], ["smoke", "--artifact"], ["smoke-replay"], ["smoke-replay", "a", "b"]]) assert.throws(() => smokePhases(args), /Usage/);
});

test("command measurement retains nonzero exit, CPU, scoped RSS and exact identities", async () => {
  const records: any[] = [];
  const result = await measuredCommand("exit-seven", process.execPath, ["-e", "const until=Date.now()+100; while(Date.now()<until){}; process.exit(7)"], { cwd: process.cwd(), record: value => records.push(value) });
  assert.equal(result.code, 7);
  const report = records[0];
  assert.equal(report.usage.exit_code, 7);
  assert.equal(report.usage.signal, null);
  assert.ok(report.usage.wall_seconds > 0);
  assert.ok(report.dispatch_wall_seconds >= report.usage.wall_seconds);
  assert.ok(report.usage.user_seconds > 0);
  assert.ok(report.usage.system_seconds >= 0);
  assert.ok(report.usage.peak_rss_bytes > 0);
  assert.equal(report.usage.peak_rss_scope, "linux_wait4_largest_single_process_not_process_group_peak");
  assert.equal(report.executable.path, process.execPath);
  assert.equal(report.executable.resolved_path, fs.realpathSync(process.execPath));
  assert.match(report.executable.sha256, /^[0-9a-f]{64}$/);
  assert.equal(report.measurement_drivers.length, 3);
});

test("measurement preserves a proxy's invoked name and uses the command environment PATH", async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "mtg-engine-proxy-test-"));
  const proxy = path.join(directory, "cargo");
  const records: any[] = [];
  try {
    fs.symlinkSync(process.execPath, proxy);
    const result = await measuredCommand("proxy", "cargo", ["-e", "process.exit(require('node:path').basename(process.argv0)==='cargo'?0:19)"], {
      cwd: directory, env: { ...process.env, PATH: `${directory}${path.delimiter}${process.env.PATH}` }, record: value => records.push(value),
    });
    assert.equal(result.code, 0);
    assert.equal(records[0].executable.path, proxy);
    assert.equal(records[0].executable.resolved_path, fs.realpathSync(process.execPath));
    assert.equal(records[0].command, proxy);
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
});

test("advisory lock remains owned across commands and releases on failure", async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "mtg-engine-lock-test-"));
  const filename = path.join(directory, "cargo.lock");
  const records: any[] = [];
  try {
    await assert.rejects(withAdvisoryLock(filename, { cwd: directory, record: value => records.push(value) }, async () => {
      assert.equal(spawnSync("flock", ["--nonblock", filename, "true"]).status, 1);
      throw new Error("focused failure");
    }), /focused failure/);
    assert.equal(records[0].kind, "lock");
    assert.ok(records[0].wait_wall_seconds >= 0);
    assert.equal(spawnSync("flock", ["--nonblock", filename, "true"]).status, 0);
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
});

test("worktrees select isolated targets and locks; a shared target still serializes", async () => {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "mtg-engine-native-test-")));
  const [first, second] = [path.join(directory, "first"), path.join(directory, "second")];
  const git = (...args: string[]) => assert.equal(spawnSync("git", args, { encoding: "utf8" }).status, 0);
  try {
    fs.mkdirSync(path.join(first, "scripts"), { recursive: true });
    for (const name of ["engine-cache.py", "engine-storage.py"]) fs.copyFileSync(fileURLToPath(new URL(name, import.meta.url)), path.join(first, "scripts", name));
    git("init", "--quiet", first);
    git("-C", first, "add", ".");
    git("-C", first, "-c", "commit.gpgsign=false", "-c", "user.name=Focused test", "-c", "user.email=focused@example.invalid", "commit", "--quiet", "-m", "fixture");
    git("-C", first, "worktree", "add", "--quiet", "--detach", second);
    const base: NodeJS.ProcessEnv = { ...process.env, MTG_ENGINE_LOCK_DIR: path.join(directory, "locks") };
    delete base.CARGO_TARGET_DIR;
    delete base.CARGO_INCREMENTAL;
    const [envA, envB] = [first, second].map(root => nativeEnvironment(root, base));
    assert.equal(envA.CARGO_TARGET_DIR, path.join(first, "target/engine"));
    assert.equal(envB.CARGO_TARGET_DIR, path.join(second, "target/engine"));
    assert.equal("CARGO_INCREMENTAL" in envA, false, "profiles choose incrementality; release stays non-incremental");
    const [lockA, lockB] = [nativeLock(first, envA), nativeLock(second, envB)];
    assert.notEqual(lockA, lockB);
    assert.equal(path.dirname(lockA), path.join(directory, "locks"));
    const shared = { ...base, CARGO_TARGET_DIR: path.join(directory, "shared") };
    assert.equal(nativeLock(first, nativeEnvironment(first, shared)), nativeLock(second, nativeEnvironment(second, shared)));
    // Holding one worktree's lock never delays another; the same target still waits.
    const records: any[] = [];
    await withAdvisoryLock(lockA, { cwd: directory, record: value => records.push(value) }, async () => {
      assert.equal(spawnSync("flock", ["--nonblock", lockA, "true"]).status, 1);
      await withAdvisoryLock(lockB, { cwd: directory, record: value => records.push(value) }, async () => {});
    });
    assert.equal(records.length, 2);
    assert.ok(records[1].wait_wall_seconds < 1);
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
});

test("worktree identity detects unstaged and untracked source changes", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "mtg-engine-identity-test-"));
  const git = (...args: string[]) => {
    const result = spawnSync("git", args, { cwd: directory });
    assert.equal(result.status, 0, result.stderr.toString());
  };
  try {
    git("init", "--quiet");
    fs.writeFileSync(path.join(directory, "source.rs"), "original\n");
    git("add", "source.rs");
    git("-c", "commit.gpgsign=false", "-c", "user.name=Focused test", "-c", "user.email=focused@example.invalid", "commit", "--quiet", "-m", "fixture");
    const original = sourceIdentity(directory);
    fs.writeFileSync(path.join(directory, "source.rs"), "changed\n");
    const unstaged = sourceIdentity(directory);
    assert.equal(original.git_head, unstaged.git_head);
    assert.notEqual(original.worktree_inventory_sha256, unstaged.worktree_inventory_sha256);
    fs.writeFileSync(path.join(directory, "new.rs"), "new\n");
    assert.notEqual(unstaged.worktree_inventory_sha256, sourceIdentity(directory).worktree_inventory_sha256);
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
});

test("cancellation cannot become success when a command handles the signal", async () => {
  const records: any[] = [];
  const signal = setTimeout(() => process.emit("SIGTERM"), 300);
  try {
    const result = await measuredCommand("cancel", process.execPath, ["-e", "process.on('SIGTERM',()=>process.exit(0)); setInterval(()=>{},1000)"], { cwd: process.cwd(), record: value => records.push(value) });
    assert.equal(result.code, 143);
    assert.equal(result.signal, "SIGTERM");
    assert.equal(records[0].code, 143);
    if (records[0].usage !== null) assert.ok(records[0].usage.wrapper_received_signals.includes(15));
  } finally { clearTimeout(signal); }
});

test("cancellation while waiting for a lock never starts protected work", async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "mtg-engine-lock-cancel-"));
  const filename = path.join(directory, "cargo.lock");
  const ready = path.join(directory, "ready");
  // Stay in the gate's cancellation group, and exec the fixture in-place so
  // normal cleanup has exactly one process to terminate (no orphan interval).
  const holder = spawn("flock", ["--exclusive", "--no-fork", filename, process.execPath, "-e", `require('node:fs').writeFileSync(${JSON.stringify(ready)},'ready'); setInterval(()=>{},1000)`], { stdio: "ignore" });
  let signal: NodeJS.Timeout | undefined;
  let entered = false;
  const records: any[] = [];
  try {
    const deadline = Date.now() + 3000;
    while (!fs.existsSync(ready)) {
      assert.ok(Date.now() < deadline, "lock holder should start");
      await new Promise(resolve => setTimeout(resolve, 10));
    }
    const group = (pid: number) => fs.readFileSync(`/proc/${pid}/stat`, "utf8").split(")").at(-1)!.trim().split(/\s+/)[2];
    assert.equal(group(holder.pid!), group(process.pid));
    signal = setTimeout(() => process.emit("SIGTERM"), 100);
    await assert.rejects(withAdvisoryLock(filename, { cwd: directory, record: value => records.push(value) }, async () => { entered = true; }), /Lock acquisition interrupted/);
    assert.equal(entered, false);
    assert.equal(records[0].code, 143);
    assert.equal(records[0].signal, "SIGTERM");
  } finally {
    clearTimeout(signal);
    holder.kill("SIGTERM");
    await new Promise(resolve => holder.once("close", resolve));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
