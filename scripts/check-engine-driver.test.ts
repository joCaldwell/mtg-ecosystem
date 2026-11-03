import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { engineCheckPhases, measuredCommand, sourceIdentity, withAdvisoryLock } from "./check-engine-driver.ts";

test("contract preflight precedes runtime while the complete normal checks remain", () => {
  const preflight = engineCheckPhases.findIndex(phase => phase.label === "contract-preflight");
  assert.ok(preflight >= 0);
  assert.ok(preflight < engineCheckPhases.findIndex(phase => phase.label === "rust-tests"));
  assert.ok(preflight < engineCheckPhases.findIndex(phase => phase.label === "clippy"));
  assert.deepEqual(engineCheckPhases.find(phase => phase.label === "rust-tests")?.args, ["test", "--workspace", "--locked"]);
  const complete = engineCheckPhases.find(phase => phase.label === "node-tests")!;
  for (const filename of ["scripts/capture-json-output.test.ts", "scripts/retain-policy-sources.test.ts", "scripts/retain-server-evidence.test.ts", "packages/game-engine/tests/contracts.test.ts"]) {
    assert.ok((complete.args as readonly string[]).includes(filename));
  }
  assert.equal("preflight" in complete, false);
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
  assert.equal(report.executable.path, fs.realpathSync(process.execPath));
  assert.match(report.executable.sha256, /^[0-9a-f]{64}$/);
  assert.equal(report.measurement_drivers.length, 3);
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
  const holder = spawn("flock", ["--exclusive", filename, process.execPath, "-e", `require('node:fs').writeFileSync(${JSON.stringify(ready)},'ready'); setInterval(()=>{},1000)`], { detached: true, stdio: "ignore" });
  let signal: NodeJS.Timeout | undefined;
  let entered = false;
  const records: any[] = [];
  try {
    const deadline = Date.now() + 3000;
    while (!fs.existsSync(ready)) {
      assert.ok(Date.now() < deadline, "lock holder should start");
      await new Promise(resolve => setTimeout(resolve, 10));
    }
    signal = setTimeout(() => process.emit("SIGTERM"), 100);
    await assert.rejects(withAdvisoryLock(filename, { cwd: directory, record: value => records.push(value) }, async () => { entered = true; }), /Lock acquisition interrupted/);
    assert.equal(entered, false);
    assert.equal(records[0].code, 143);
    assert.equal(records[0].signal, "SIGTERM");
  } finally {
    clearTimeout(signal);
    if (holder.pid) process.kill(-holder.pid, "SIGTERM");
    await new Promise(resolve => holder.once("close", resolve));
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
