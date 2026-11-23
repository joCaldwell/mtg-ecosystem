import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { digest, engineCheckPlans, nativeEnvironment, nativeLock, parseCheckArguments, pinnedRust, runCheckPhases, sourceIdentity, withAdvisoryLock } from "./check-engine-driver.ts";

const root = fileURLToPath(new URL("../", import.meta.url));
const { mode, planOnly, help } = parseCheckArguments(process.argv.slice(2));
const plan = engineCheckPlans[mode];
const { phases, needsCargo } = plan;
if (help) {
  console.log("Usage: npm run check:engine -- [--check|--gate|--tooling] [--plan]");
  console.log("Default: --check. --plan prints commands and omissions without executing them.");
  for (const [name, value] of Object.entries(engineCheckPlans)) console.log(`${name}: ${value.coverage}\n  Omits: ${value.omitted}`);
  process.exit(0);
}
if (planOnly) {
  console.log(JSON.stringify({ mode, ...plan, phases }, null, 2));
  process.exit(0);
}
console.error(`Engine check mode: ${mode}\n${plan.coverage}\nOmitted: ${plan.omitted}`);
const version = needsCargo ? pinnedRust(root) : null;
const report = process.env.MTG_ENGINE_TIMING_FILE
  ? path.resolve(process.env.MTG_ENGINE_TIMING_FILE)
  : path.join(root, "generated", "engine-check-timing", `${new Date().toISOString().replaceAll(":", "-")}-${process.pid}.jsonl`);
fs.mkdirSync(path.dirname(report), { recursive: true });
const reportFd = fs.openSync(report, "wx");
const record = (value: unknown) => fs.writeSync(reportFd, `${JSON.stringify(value)}\n`);
// Per-worktree target; Cargo profiles choose incrementality unless explicitly set.
const env: NodeJS.ProcessEnv = needsCargo ? nativeEnvironment(root) : { ...process.env };
const options = { cwd: root, record, env };
const lock = needsCargo ? nativeLock(root, env) : null;
console.error(`Engine timing evidence: ${report}`);
try {
  const run = async () => {
    const source = sourceIdentity(root);
    record({ kind: "freeze", mode, coverage: plan.coverage, omitted: plan.omitted, phases, source, driver_sha256: digest(fs.readFileSync(fileURLToPath(import.meta.url))), node: process.version, rust: version, cargo_incremental: env.CARGO_INCREMENTAL ?? "profile default", cargo_build_jobs: env.CARGO_BUILD_JOBS ?? "cargo default", cargo_target_dir: env.CARGO_TARGET_DIR ?? path.join(root, "target"), cargo_lock: lock });
    try {
      const result = await runCheckPhases(phases, options);
      record({ kind: "phases-complete", mode, ...result });
      process.exitCode = result.code;
    } finally {
      const finalSource = sourceIdentity(root);
      const unchanged = JSON.stringify(finalSource) === JSON.stringify(source);
      record({ kind: "source-after", source: finalSource, unchanged });
      if (!unchanged) throw new Error("Engine check source changed during execution; the result does not describe one unchanged source inventory.");
    }
  };
  // Own the selected target's lock across nested Node Cargo calls; never wrap
  // check:engine in another acquisition of it. The tooling mode needs no lock.
  if (lock) await withAdvisoryLock(lock, options, run);
  else await run();
} finally { fs.closeSync(reportFd); }
