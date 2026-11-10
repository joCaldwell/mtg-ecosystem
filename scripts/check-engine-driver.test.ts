import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { engineCheckPlans, parseCheckArguments, runCheckPhases, measuredCommand, sourceIdentity, withAdvisoryLock } from "./check-engine-driver.ts";

test("default coverage is bounded and deeper runtime work is explicit", () => {
  assert.equal(parseCheckArguments([]).mode, "fast");
  const fast = engineCheckPlans.fast.phases;
  assert.ok(fast.some(phase => phase.label === "compiler-tests"));
  assert.ok(fast.some(phase => phase.label === "rust-check"));
  assert.ok(fast.some(phase => phase.label === "archive-parser"));
  assert.ok(!fast.some(phase => ["clippy", "rust-tests", "node-tests", "contract-preflight"].includes(phase.label)));
  assert.ok(!fast.some(phase => phase.args.includes("--release")));
  assert.equal(engineCheckPlans.source.phases.some(phase => phase.command === "cargo"), false);
  const full = engineCheckPlans.full.phases;
  assert.deepEqual(full.find(phase => phase.label === "rust-tests")?.args, ["test", "--workspace", "--locked"]);
  assert.equal(full.filter(phase => phase.args.includes("packages/game-engine/tests/contracts.test.ts")).length, 1);
  assert.equal("preflight" in full.find(phase => phase.label === "node-tests")!, false);
  assert.match(engineCheckPlans.fast.omitted, /Core\/simulator\/server runtime tests/);
  for (const plan of Object.values(engineCheckPlans)) {
    assert.equal(plan.phases.filter(phase => phase.args.includes("scripts/test-engine-storage.py")).length, 1);
    assert.equal(plan.phases.filter(phase => phase.args.includes("scripts/test-retention-workspace.py")).length, 1);
    assert.ok(plan.phases.some(phase => phase.args.includes("scripts/retention-workspace.test.ts")));
  }
});

test("mode selection rejects typos and conflicting modes before launching commands", () => {
  assert.deepEqual(parseCheckArguments(["--contracts", "--plan"]), { mode: "contracts", planOnly: true, help: false });
  assert.throws(() => parseCheckArguments(["--ful"]), /Unknown/);
  assert.throws(() => parseCheckArguments(["--full", "--fast"]), /exactly one/);
  assert.throws(() => parseCheckArguments(["--full", "--full"]), /exactly one/);
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
    assert.deepEqual(result, { code: code ?? 1, completed: [], failed: "format" });
  }
  const cohorts: string[] = [];
  const result = await runCheckPhases(engineCheckPlans.contracts.phases, {
    cwd: process.cwd(), record: () => {}, env: { MTG_ENGINE_CONTRACT_PREFLIGHT: "0" },
  }, async (label, command, args, options) => {
    cohorts.push(options.env!.MTG_ENGINE_CONTRACT_PREFLIGHT!);
    return { code: 0, signal: null };
  });
  assert.deepEqual(cohorts, ["0", "0", "0", "1"]);
  assert.deepEqual(result, { code: 0, completed: ["tooling-tests", "storage-safety", "retention-safety", "contract-preflight"], failed: null });
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
