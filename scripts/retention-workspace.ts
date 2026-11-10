// The Python supervisor owns process cancellation, disk bounds and scratch cleanup.
import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";

export function retentionScratch(): string {
  const scratch = process.env.MTG_RETENTION_SCRATCH;
  if (!scratch || !path.isAbsolute(scratch) || fs.realpathSync(scratch) !== scratch) {
    throw new Error("Use npm run retain:engine / retain:server for bounded retention scratch");
  }
  const owner = JSON.parse(fs.readFileSync(path.join(scratch, "owner.json"), "utf8"));
  if (owner.version !== 1 || owner.pid !== process.ppid) throw new Error("Retention supervisor ownership mismatch");
  return scratch;
}

/** Keep the exact executable in evidence; fresh-target is disposable afterwards. */
export function retainExecutable(source: string, store: string, name: string): string {
  if (path.basename(name) !== name || name === "." || name === "..") throw new Error("Invalid executable name");
  const directory = path.join(store, "bin");
  fs.mkdirSync(directory, { recursive: true });
  if (fs.realpathSync(directory) !== path.resolve(directory)) throw new Error("Retained binary directory is a symlink");
  const destination = path.join(directory, name);
  fs.copyFileSync(source, destination, fs.constants.COPYFILE_EXCL);
  fs.chmodSync(destination, fs.statSync(source).mode & 0o777);
  const hash = (file: string) => createHash("sha256").update(fs.readFileSync(file)).digest("hex");
  if (hash(source) !== hash(destination)) throw new Error("Retained executable copy differs");
  return destination;
}
