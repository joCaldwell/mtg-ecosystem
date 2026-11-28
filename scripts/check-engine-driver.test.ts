import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { testThreads, engineCheckPlans, parseCheckArguments, runCheckPhases, measuredCommand, nativeEnvironment, nativeLock, sourceIdentity, withAdvisoryLock } from "./check-engine-driver.ts";

test("one feature shape, a production-only dead-code check and no release build in any plan", () => {
  const { mode } = parseCheckArguments([]);
  assert.equal(mode, "check");
  const shape = ["--locked", "--workspace", "--features", "mtg-core/testkit"];
  const cargo = (name: keyof typeof engineCheckPlans) => engineCheckPlans[name].phases.filter(phase => phase.command === "cargo").map(phase => phase.args);
  assert.deepEqual(cargo("check"), [
    ["fmt", "--all", "--", "--check"],
    ["check", ...shape, "--all-targets"],
    ["check", "--locked", "-p", "mtg-server", "-p", "mtg-sim"],
  ]);
  assert.deepEqual(cargo("gate"), [
    ["fmt", "--all", "--", "--check"],
    ["clippy", ...shape, "--all-targets", "--", "-D", "warnings"],
    ["check", "--locked", "-p", "mtg-server", "-p", "mtg-sim"],
    ["test", ...shape, "--tests", "--", testThreads],
    ["test", ...shape, "--doc", "--", testThreads],
  ]);
  const gate = engineCheckPlans.gate.phases;
  assert.deepEqual(gate.find(phase => phase.label === "archive-parser")?.args, ["--test", "packages/oracle-parser/test/engine-archive-agreement.test.ts"]);
  for (const plan of Object.values(engineCheckPlans)) {
    assert.ok(!plan.phases.some(phase => phase.args.includes("--release") || phase.args.includes("run")));
    assert.ok(!plan.phases.some(phase => phase.args.includes("--profile")));
  }
  assert.equal(engineCheckPlans.tooling.phases.some(phase => phase.command === "cargo"), false);
});

test("native plans admit the cache under their lock; gate and tooling retain every harness suite", () => {
  const suites = [
    ["tooling-tests", "node", ["--test", "scripts/check-engine-driver.test.ts", "scripts/engine-environment.test.ts"]],
    ["storage-safety", "python3", ["-B", "scripts/test-engine-storage.py"]],
    ["worktree-safety", "python3", ["-B", "scripts/test-engine-worktrees.py"]],
    ["structure-tests", "python3", ["-B", "scripts/test-engine-structure.py"]],
    ["cache-tests", "python3", ["-B", "scripts/test-engine-cache.py"]],
    ["deck-harness-tests", "python3", ["-B", "scripts/test-engine-decks.py"]],
    ["rework-sweep-tests", "python3", ["-B", "scripts/test-rework-sweep.py"]],
    ["archive-cut-tests", "python3", ["-B", "scripts/test-compare-archive-cut.py"]],
    ["stage-deck-tests", "python3", ["-B", "scripts/test-stage-deck.py"]],
    ["documentation-tests", "python3", ["-B", "scripts/test-check-docs.py"]],
  ];
  for (const [mode, plan] of Object.entries(engineCheckPlans)) {
    const native = mode !== "tooling";
    const selfTests = mode !== "check";
    assert.equal(plan.needsCargo, native);
    assert.equal(plan.phases.filter(phase => phase.label === "cache-preflight").length, native ? 1 : 0);
    if (native) assert.deepEqual(plan.phases[0], { label: "cache-preflight", command: "python3", args: ["-B", "scripts/engine-cache.py", "check"], lock: true });
    // Only cache admission may receive the held lock descriptor.
    assert.deepEqual(plan.phases.filter(phase => phase.lock).map(phase => phase.label), native ? ["cache-preflight"] : []);
    assert.equal(plan.phases.filter(phase => phase.args.includes("scripts/engine-structure-check.py")).length, 1);
    assert.equal(plan.phases.filter(phase => phase.args.includes("scripts/check-docs.py")).length, 1);
    assert.deepEqual(plan.phases.filter(phase => suites.some(([label]) => label === phase.label)).map(phase => [phase.label, phase.command, phase.args]), selfTests ? suites : []);
  }
  for (const [, , args] of suites) {
    const script = (args as string[]).find(arg => arg.startsWith("scripts/"))!;
    assert.ok(fs.existsSync(fileURLToPath(new URL(`../${script}`, import.meta.url))), script);
  }
  const labels = (mode: keyof typeof engineCheckPlans) => engineCheckPlans[mode].phases.map(phase => phase.label);
  const guards = ["structure-guard", "documentation-guard", "generated-ast-guard"];
  assert.deepEqual(labels("check"), ["cache-preflight", ...guards, "format", "rust-check", "production-check"]);
  assert.deepEqual(labels("gate"), ["cache-preflight", ...guards, "archive-parser", "format", "clippy", "production-check", "workspace-tests", "doc-tests", ...suites.map(([label]) => label)]);
  assert.deepEqual(labels("tooling"), [...suites.map(([label]) => label), ...guards]);
  const scripts = JSON.parse(fs.readFileSync(new URL("../package.json", import.meta.url), "utf8")).scripts;
  assert.equal(scripts["check:engine"], "sh scripts/engine-env.sh node scripts/check-engine.ts");
  assert.equal(scripts["check:engine:gate"], "sh scripts/engine-env.sh node scripts/check-engine.ts --gate");
  assert.equal(scripts["check:engine:tooling"], "sh scripts/engine-env.sh node scripts/check-engine.ts --tooling");
  assert.deepEqual(Object.keys(scripts).filter(name => name.startsWith("check:engine")).sort(), ["check:engine", "check:engine:gate", "check:engine:tooling"]);
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
    assert.match(result.stderr, /Omitted: Rust formatting, compilation, Clippy and tests/);
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
    assert.deepEqual(JSON.parse(implicit.stdout), { mode: "check", ...engineCheckPlans.check });
    assert.deepEqual(implicit.calls, []);
    assert.deepEqual(implicit.records, []);
    const help = run(["--tooling", "--help"]);
    assert.equal(help.status, 0, help.stderr);
    assert.match(help.stdout, /Default: --check/);
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

test("mode selection rejects typos and conflicting modes before launching commands", () => {
  assert.deepEqual(parseCheckArguments(["--gate", "--plan"]), { mode: "gate", planOnly: true, help: false });
  assert.deepEqual(parseCheckArguments(["--tooling"]), { mode: "tooling", planOnly: false, help: false });
  for (const retired of ["--quick", "--fast", "--core", "--source", "--contracts", "--full", "--gat"]) assert.throws(() => parseCheckArguments([retired]), /Unknown/);
  assert.throws(() => parseCheckArguments(["--gate", "--check"]), /exactly one/);
  assert.throws(() => parseCheckArguments(["--gate", "--gate"]), /exactly one/);
});

test("phase failure stops subsequent work and the caller environment passes through", async () => {
  for (const code of [7, 143, null]) {
    const calls: any[] = [];
    const result = await runCheckPhases(engineCheckPlans.gate.phases, {
      cwd: process.cwd(), record: () => {}, env: { CARGO_INCREMENTAL: "0" },
    }, async (label, command, args, options) => {
      calls.push({ label, env: options.env });
      return { code, signal: null };
    });
    assert.equal(calls.length, 1);
    assert.equal(calls[0].env.CARGO_INCREMENTAL, "0");
    assert.deepEqual(result, { code: code ?? 1, completed: [], failed: "cache-preflight" });
  }
  const result = await runCheckPhases(engineCheckPlans.check.phases, { cwd: process.cwd(), record: () => {} }, async () => ({ code: 0, signal: null }));
  assert.deepEqual(result, { code: 0, completed: engineCheckPlans.check.phases.map(phase => phase.label), failed: null });
});

test("only lock phases inherit the held descriptor, as a verifiable fd 3", async () => {
  const seen: any[] = [];
  await runCheckPhases(engineCheckPlans.check.phases, { cwd: process.cwd(), record: () => {}, env: {}, lockFd: 99 }, async (label, command, args, options) => {
    seen.push([label, options.lockFd, options.env?.MTG_ENGINE_LOCK_FD]);
    return { code: 0, signal: null };
  });
  assert.deepEqual(seen.filter(([, fd]) => fd !== undefined), [["cache-preflight", 99, "3"]]);
  assert.ok(seen.filter(([label]) => label !== "cache-preflight").every(([, fd, env]) => fd === undefined && env === undefined));
  // End to end: the measured child (through the rusage wrapper) receives the
  // exact held description and can re-flock it without blocking.
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "mtg-engine-lockfd-test-"));
  const filename = path.join(directory, "cargo.lock");
  const probe = `import fcntl,os,sys\na,b=os.fstat(3),os.stat(sys.argv[1])\nassert (a.st_dev,a.st_ino)==(b.st_dev,b.st_ino)\nfcntl.flock(3,fcntl.LOCK_EX|fcntl.LOCK_NB)`;
  try {
    await withAdvisoryLock(filename, { cwd: directory, record: () => {} }, async descriptor => {
      const held = await measuredCommand("held", "python3", ["-c", probe, filename], { cwd: directory, record: () => {}, lockFd: descriptor });
      assert.equal(held.code, 0);
      const missing = await measuredCommand("missing", "python3", ["-c", probe, filename], { cwd: directory, record: () => {} });
      assert.notEqual(missing.code, 0);
    });
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
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

test("Cargo must execute each named harness and both authority doctests", async () => {
  const names = ["cards", "host", "mtg_compiler", "mtg_contracts", "mtg_core", "mtg_env", "mtg_observe", "mtg_view", "mtg_server", "mtg_sim", "recovery", "rules"];
  const summary = (passed: number, ignored = 0) =>
    `test result: ok. ${passed} passed; 0 failed; ${ignored} ignored; 0 measured; 0 filtered out`;
  const harness = (name: string, passed = 1) => [
    `Running ${name.startsWith("mtg_") ? "unittests src/lib.rs" : `tests/${name}/main.rs`} (target/debug/deps/${name}-abc123)`,
    summary(passed, name === "host" ? 1 : 0),
  ];
  const ordinary = names.flatMap(name => harness(name));
  const core = "Doc-tests mtg_core";
  const example = (file: string, type: string) =>
    `test src/engine/${file}.rs - engine::${file}::${type} (line 15) - compile fail ... ok`;
  const action = example("api::decision::model", "Rules");
  const session = example("player_session", "PlayerSession");
  for (const [label, lines, expected] of [
    ["workspace-tests", ordinary, 0],
    ["workspace-tests", names.filter(n => n !== "recovery").flatMap(n => harness(n)), 1],
    ["workspace-tests", [...ordinary.slice(0, -2), ...harness("host")], 1],
    ["workspace-tests", names.flatMap(n => harness(n, n === "host" ? 0 : 1)), 1],
    ["workspace-tests", [...ordinary, summary(0)], 1],
    ["workspace-tests", [], 1],
    ["doc-tests", ["Doc-tests mtg_compiler", summary(0), core, action, session, summary(2)], 0],
    ["doc-tests", [core, action, summary(1)], 1],
    ["doc-tests", [core, session, session, summary(2)], 1],
    ["doc-tests", [core, action, session, summary(1)], 1],
  ] as const) {
    // Match Cargo's real split streams, including the banner/result order.
    const emit = 'const fs = require("node:fs"); for (const line of JSON.parse(process.argv[1])) fs.writeSync(/^(Running|Doc-tests)/.test(line) ? 2 : 1, line + "\\n");';
    const result = await measuredCommand(label, process.execPath, ["-e", emit, JSON.stringify(lines)], { cwd: process.cwd(), record() {} });
    assert.equal(result.code, expected);
  }
});
