import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { nativeEnvironment, pinnedRust, runCheckPhases, smokePhases, sourceIdentity, withAdvisoryLock } from "./check-engine-driver.ts";

const root = fileURLToPath(new URL("../", import.meta.url));
const phases = smokePhases(process.argv.slice(2));
const rust = pinnedRust(root);
const env = nativeEnvironment(root);
const report = path.join(root, "generated", "engine-check-timing", `smoke-${new Date().toISOString().replaceAll(":", "-")}-${process.pid}.jsonl`);
fs.mkdirSync(path.dirname(report), { recursive: true });
const fd = fs.openSync(report, "wx");
const options = { cwd: root, env, record: (value: unknown) => fs.writeSync(fd, `${JSON.stringify(value)}\n`) };
console.error(`Smoke timing evidence: ${report}`);
try {
  await withAdvisoryLock(path.join(os.homedir(), ".local/share/mtg-ecosystem/engine-verification/cargo.lock"), options, async () => {
    const source = sourceIdentity(root);
    options.record({ kind: "freeze", source, rust, phases, cargo_target_dir: env.CARGO_TARGET_DIR, cargo_incremental: env.CARGO_INCREMENTAL });
    try {
      const result = await runCheckPhases(phases, options);
      options.record({ kind: "phases-complete", ...result });
      process.exitCode = result.code;
    } finally {
      const after = sourceIdentity(root);
      const unchanged = JSON.stringify(after) === JSON.stringify(source);
      options.record({ kind: "source-after", source: after, unchanged });
      if (!unchanged) throw new Error("Smoke source changed during execution; rerun on a stable inventory.");
    }
  });
} finally { fs.closeSync(fd); }
