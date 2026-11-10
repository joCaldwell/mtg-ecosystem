import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { digest, engineCheckPlans, parseCheckArguments, runCheckPhases, sourceIdentity, withAdvisoryLock } from "./check-engine-driver.ts";

const root = fileURLToPath(new URL("../", import.meta.url));
const { mode, planOnly, help } = parseCheckArguments(process.argv.slice(2));
const plan = engineCheckPlans[mode];
if (help) {
  console.log("Usage: npm run check:engine -- [--fast|--source|--contracts|--full] [--plan]");
  console.log("Default: --fast. --plan prints commands and omissions without executing them.");
  for (const [name, value] of Object.entries(engineCheckPlans)) console.log(`${name}: ${value.coverage}\n  Omits: ${value.omitted}`);
  process.exit(0);
}
if (planOnly) {
  console.log(JSON.stringify({ mode, ...plan }, null, 2));
  process.exit(0);
}
console.error(`Engine check mode: ${mode}\n${plan.coverage}\nOmitted: ${plan.omitted}`);
const needsCargo = mode !== "source";
const version = needsCargo ? spawnSync("rustc", ["--version"], { encoding: "utf8" }) : null;
if (version && (version.status !== 0 || !version.stdout.startsWith("rustc 1.99.0 "))) {
  console.error("Engine checks require the pinned Rust 1.99.0 toolchain.");
  process.exit(1);
}
const report = process.env.MTG_ENGINE_TIMING_FILE
  ? path.resolve(process.env.MTG_ENGINE_TIMING_FILE)
  : path.join(root, "generated", "engine-check-timing", `${new Date().toISOString().replaceAll(":", "-")}-${process.pid}.jsonl`);
fs.mkdirSync(path.dirname(report), { recursive: true });
const reportFd = fs.openSync(report, "wx");
const record = (value: unknown) => fs.writeSync(reportFd, `${JSON.stringify(value)}\n`);
// Avoid growing multi-GiB incremental caches in routine checks. A caller can
// explicitly opt in; record the effective value instead of hiding that tradeoff.
const env: NodeJS.ProcessEnv = { ...process.env, CARGO_INCREMENTAL: process.env.CARGO_INCREMENTAL ?? "0" };
const options = { cwd: root, record, env };
const locks = path.join(os.homedir(), ".local/share/mtg-ecosystem/engine-verification");
console.error(`Engine timing evidence: ${report}`);
try {
  const run = async () => {
    const source = sourceIdentity(root);
    record({ kind: "freeze", mode, coverage: plan.coverage, omitted: plan.omitted, phases: plan.phases, source, driver_sha256: digest(fs.readFileSync(fileURLToPath(import.meta.url))), node: process.version, rust: version?.stdout.trim() ?? null, cargo_incremental: env.CARGO_INCREMENTAL, cargo_target_dir: env.CARGO_TARGET_DIR ?? path.join(root, "target") });
    try {
      const result = await runCheckPhases(plan.phases, options);
      record({ kind: "phases-complete", mode, ...result });
      process.exitCode = result.code;
    } finally {
      const finalSource = sourceIdentity(root);
      const unchanged = JSON.stringify(finalSource) === JSON.stringify(source);
      record({ kind: "source-after", source: finalSource, unchanged });
      if (!unchanged) throw new Error("Engine check source changed during execution; the result does not describe one unchanged source inventory.");
    }
  };
  // Own the Cargo lock across nested Node Cargo calls; never wrap check:engine
  // in another acquisition of this lock. Source-only mode needs no Cargo lock.
  if (needsCargo) await withAdvisoryLock(path.join(locks, "cargo.lock"), options, run);
  else await run();
} finally { fs.closeSync(reportFd); }
