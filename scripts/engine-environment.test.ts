import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { inspectEngine } from "./engine-doctor.ts";

const root = fileURLToPath(new URL("../", import.meta.url));
function fixture(t: { after: (fn: () => void) => void }) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "engine environment "));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  fs.mkdirSync(path.join(directory, "scripts"));
  for (const name of [".node-version", "rust-toolchain.toml", "package.json", "package-lock.json", "scripts/engine-env.sh", "scripts/engine-bootstrap.sh", "scripts/engine-bootstrap-install.sh", "scripts/engine-doctor.ts"]) {
    fs.copyFileSync(path.join(root, name), path.join(directory, name));
  }
  return directory;
}
const versions: Record<string, string> = {
  node: "v26.11.1", rustc: "rustc 1.99.0 (test)", cargo: "cargo 1.99.0 (test)",
  npm: "12.2.1", rustfmt: "rustfmt 1.10.0", "cargo-clippy": "clippy 0.1.99", python3: "Python 3.14.9", flock: "flock 2.42.4",
};
function run(command: string, args: string[]) {
  if (command !== "git") return { status: 0, stdout: versions[command] ?? "" };
  if (args[0] === "--version") return { status: 0, stdout: "git version 2.56.0" };
  if (args[0] === "rev-list") return { status: 0, stdout: "0\t0\n" };
  return { status: 0, stdout: "local-reference" };
}
const freeBytes = () => 8 * 1024 ** 3;

test("preflight checks tools and disk without requiring or creating dependency/cache trees", t => {
  const directory = fixture(t);
  const before = fs.readdirSync(directory);
  const checks = inspectEngine({ root: directory, run, freeBytes, preflight: true });
  assert.equal(checks.some(check => check.status === "fail"), false);
  assert.deepEqual(fs.readdirSync(directory), before);
  assert.equal(fs.existsSync(path.join(directory, "target")), false);
});

test("wrong toolchain and exhausted disk give failures and repair instructions", t => {
  const directory = fixture(t);
  const checks = inspectEngine({ root: directory, preflight: true, freeBytes: () => 0,
    run: (command, args) => command === "rustc" ? { status: 0, stdout: "rustc 1.98.0" } : run(command, args) });
  for (const name of ["rustc", "worktree disk", "Cargo disk"]) {
    const check = checks.find(item => item.name === name);
    assert.equal(check?.status, "fail");
    assert.ok(check?.fix);
  }
});

test("missing locked dependencies fail after installation preflight", t => {
  const checks = inspectEngine({ root: fixture(t), run, freeBytes });
  assert.deepEqual(checks.filter(check => check.name.startsWith("dependency ")).map(check => check.status), ["fail", "fail", "fail"]);
  assert.equal(checks.find(check => check.name === "workspace oracle-parser")?.status, "fail");
});

test("wrong installed dependency version and foreign workspace links fail", t => {
  const directory = fixture(t);
  fs.mkdirSync(path.join(directory, "node_modules/typescript"), { recursive: true });
  fs.writeFileSync(path.join(directory, "node_modules/typescript/package.json"), '{"version":"0.0.1"}');
  fs.mkdirSync(path.join(directory, "packages/oracle-parser"), { recursive: true });
  fs.mkdirSync(path.join(directory, "node_modules/@mtg-ecosystem"), { recursive: true });
  fs.symlinkSync(path.join(root, "packages/oracle-parser"), path.join(directory, "node_modules/@mtg-ecosystem/oracle-parser"));
  const checks = inspectEngine({ root: directory, run, freeBytes });
  assert.equal(checks.find(check => check.name === "dependency typescript")?.status, "fail");
  assert.equal(checks.find(check => check.name === "workspace oracle-parser")?.status, "fail");
});

test("missing or diverged local upstream is advisory and no fetch/config occurs", t => {
  const directory = fixture(t);
  for (const missing of [true, false]) {
    const checks = inspectEngine({ root: directory, preflight: true, freeBytes,
      run: (command, args) => {
        assert.ok(!["fetch", "remote", "config"].includes(args[0]));
        if (command === "git" && missing && args.includes("@{upstream}")) return { status: 1, stdout: "" };
        if (command === "git" && args[0] === "rev-list") return { status: 0, stdout: "2\t3\n" };
        return run(command, args);
      } });
    assert.equal(checks.find(check => check.name === "upstream")?.status, "warn");
    assert.equal(checks.some(check => check.status === "fail"), false);
  }
});

test("doctor does not expose arbitrary child output or environment secrets", t => {
  const secret = "secret-value-that-must-not-appear";
  const checks = inspectEngine({ root: fixture(t), preflight: true, freeBytes,
    env: { ...process.env, NPM_TOKEN: secret },
    run: () => ({ status: 1, stdout: secret }) });
  assert.ok(!JSON.stringify(checks).includes(secret));
});

test("launcher preserves argument boundaries, explicit cache and loader environment", () => {
  const result = spawnSync("sh", [path.join(root, "scripts/engine-env.sh"), "node", "-e",
    'console.log(JSON.stringify({args:process.argv.slice(1), target:process.env.CARGO_TARGET_DIR, incremental:process.env.CARGO_INCREMENTAL, rust:process.env.RUSTUP_TOOLCHAIN, auto:process.env.RUSTUP_AUTO_INSTALL, loader:process.env.LD_LIBRARY_PATH}))',
    "one two", "$(never-run)", "semi;colon"], {
    encoding: "utf8", env: { ...process.env, CARGO_TARGET_DIR: "/tmp/explicit engine target", CARGO_INCREMENTAL: "1", LD_LIBRARY_PATH: "/tmp/previous-loader-path" },
  });
  assert.equal(result.status, 0, result.stderr);
  const actual = JSON.parse(result.stdout);
  assert.deepEqual(actual.args, ["one two", "$(never-run)", "semi;colon"]);
  assert.equal(actual.target, "/tmp/explicit engine target");
  assert.equal(actual.incremental, "1");
  assert.equal(actual.rust, "1.99.0");
  assert.equal(actual.auto, "0");
  assert.ok(actual.loader.endsWith("/tmp/previous-loader-path"));
});

test("missing explicit toolchain blocks bootstrap before dependency installation", t => {
  const directory = fixture(t);
  const result = spawnSync("sh", [path.join(directory, "scripts/engine-bootstrap.sh")], {
    encoding: "utf8", env: { ...process.env, MTG_ENGINE_TOOLCHAIN_ROOT: path.join(directory, "absent") },
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /toolchain override is unavailable/);
  assert.equal(fs.existsSync(path.join(directory, "node_modules")), false);
});

test("launcher rejects a mismatched Node pin without executing the command", t => {
  const directory = fixture(t);
  fs.writeFileSync(path.join(directory, ".node-version"), "0.0.1\n");
  const result = spawnSync("sh", [path.join(directory, "scripts/engine-env.sh"), "node", "-e", 'console.log("should-not-run")'], { encoding: "utf8" });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /require Node 0.0.1/);
  assert.equal(result.stdout, "");
});

test("unavailable rustup toolchain does not block source-only commands", t => {
  const directory = fixture(t);
  fs.copyFileSync(path.join(root, "scripts/engine-cache.py"), path.join(directory, "scripts/engine-cache.py"));
  const bundle = path.join(directory, "tools");
  fs.mkdirSync(path.join(bundle, "usr/bin"), { recursive: true });
  fs.symlinkSync(process.execPath, path.join(bundle, "usr/bin/node"));
  fs.writeFileSync(path.join(bundle, "usr/bin/rustc"), "#!/bin/sh\nexit 1\n", { mode: 0o755 });
  const env: NodeJS.ProcessEnv = { ...process.env, MTG_ENGINE_TOOLCHAIN_ROOT: bundle };
  delete env.CARGO_TARGET_DIR;
  const result = spawnSync("sh", [path.join(directory, "scripts/engine-env.sh"), "node", "-e", 'console.log("source-only works")'], { env, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), "source-only works");
});

test("doctor JSON includes a versioned failure envelope for bad arguments and configuration", t => {
  const directory = fixture(t);
  const script = path.join(directory, "scripts/engine-doctor.ts");
  fs.writeFileSync(path.join(directory, "rust-toolchain.toml"), "invalid-config-secret-must-not-leak");
  for (const [args, status, code] of [
    [["--json", "--typo"], 2, "invalid_arguments"],
    [["--json"], 1, "configuration_unreadable"],
  ] as const) {
    const result = spawnSync(process.execPath, [script, ...args], { encoding: "utf8" });
    assert.equal(result.status, status);
    const report = JSON.parse(result.stdout);
    assert.equal(report.format_version, 1);
    assert.equal(report.healthy, false);
    assert.deepEqual(report.checks, []);
    assert.equal(report.error.code, code);
    assert.equal(result.stderr, "");
    assert.ok(!result.stdout.includes("invalid-config-secret-must-not-leak"));
  }
});
