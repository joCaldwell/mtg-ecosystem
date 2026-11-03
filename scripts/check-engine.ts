import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { digest, engineCheckPhases, measuredCommand, sourceIdentity, withAdvisoryLock } from "./check-engine-driver.ts";

const root = fileURLToPath(new URL("../", import.meta.url));
const version = spawnSync("rustc", ["--version"], { encoding: "utf8" });
if (version.status !== 0 || !version.stdout.startsWith("rustc 1.99.0 ")) {
  console.error("Engine checks require the pinned Rust 1.99.0 toolchain.");
  process.exit(1);
}
const report = process.env.MTG_ENGINE_TIMING_FILE
  ? path.resolve(process.env.MTG_ENGINE_TIMING_FILE)
  : path.join(root, "generated", "engine-check-timing", `${new Date().toISOString().replaceAll(":", "-")}-${process.pid}.jsonl`);
fs.mkdirSync(path.dirname(report), { recursive: true });
const reportFd = fs.openSync(report, "wx");
const record = (value: unknown) => fs.writeSync(reportFd, `${JSON.stringify(value)}\n`);
const options = { cwd: root, record };
const locks = path.join(os.homedir(), ".local/share/mtg-ecosystem/engine-verification");
console.error(`Engine timing evidence: ${report}`);
try {
  // The designated verifier owns full-gate.lock across multi-phase acceptance.
  // check-engine owns cargo.lock itself, including nested Node Cargo calls; do
  // not wrap this command in another acquisition of that same advisory lock.
  await withAdvisoryLock(path.join(locks, "cargo.lock"), options, async () => {
    const source = sourceIdentity(root);
    record({ kind: "freeze", source, driver_sha256: digest(fs.readFileSync(fileURLToPath(import.meta.url))), node: process.version, rust: version.stdout.trim() });
    try {
      for (const phase of engineCheckPhases) {
        const result = await measuredCommand(phase.label, phase.command, [...phase.args], {
          ...options,
          // Explicitly reset the cohort for the complete normal run, even if
          // a caller inherited a preflight-only development environment.
          env: { ...process.env, MTG_ENGINE_CONTRACT_PREFLIGHT: "preflight" in phase ? "1" : "0" },
        });
        if (result.code !== 0) {
          process.exitCode = result.code ?? 1;
          break;
        }
      }
    } finally {
      const finalSource = sourceIdentity(root);
      const unchanged = JSON.stringify(finalSource) === JSON.stringify(source);
      record({ kind: "source-after", source: finalSource, unchanged });
      if (!unchanged) throw new Error("Engine check source changed during execution; this is not a frozen acceptance result.");
    }
  });
} finally { fs.closeSync(reportFd); }
