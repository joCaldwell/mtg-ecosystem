// JSON summaries can exceed a pipe's maxBuffer while the retained bundle itself
// is much larger. Capture child stdout directly to disk; only parse its summary.
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export interface JsonCaptureOptions {
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  temporaryRoot?: string;
}
export function captureJsonOutput<T = unknown>(command: string, args: string[], options: JsonCaptureOptions = {}): T {
  const temporary = fs.mkdtempSync(path.join(options.temporaryRoot ?? os.tmpdir(), "mtg-engine-json-"));
  let descriptor: number | undefined;
  try {
    const output = path.join(temporary, "stdout.json");
    descriptor = fs.openSync(output, "wx");
    // A file descriptor bypasses spawnSync's buffered-pipe limit. Inherit
    // stderr so progress/errors remain visible and cannot fill another pipe.
    const result = spawnSync(command, args, {
      cwd: options.cwd, env: options.env ?? process.env,
      stdio: ["ignore", descriptor, "inherit"],
    });
    fs.closeSync(descriptor); descriptor = undefined;
    if (result.error) throw new Error(`${command} could not run: ${result.error.message}`, { cause: result.error });
    if (result.status !== 0) throw new Error(`${command} failed: ${result.signal ?? result.status}`);
    return JSON.parse(fs.readFileSync(output, "utf8")) as T;
  } finally {
    if (descriptor !== undefined) fs.closeSync(descriptor);
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}
