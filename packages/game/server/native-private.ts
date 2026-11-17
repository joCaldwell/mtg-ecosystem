import { constants, openSync, closeSync, fsyncSync, writeFileSync, readFileSync, lstatSync, mkdirSync, chmodSync, renameSync } from "node:fs";
import { dirname, isAbsolute, parse, resolve, sep } from "node:path";
import type { NativeFileDetail } from "./native-diagnostics.ts";

export type NativeFileObserver = (detail: NativeFileDetail) => void | Promise<void>;
/** Failure observation never substitutes an exception or waits on a callback. */
export function observePrivateFailure(observe: NativeFileObserver | undefined, detail: NativeFileDetail): void {
  if (!observe) return;
  try { void Promise.resolve(observe(detail)).catch(() => {}); } catch { /* observational only */ }
}
/** Private host storage; excludes a hostile concurrent same-UID process, as does
 * the native registry. Never repairs or deletes an existing non-private path. */
export function privateDirectory(path: string): void {
  if (!isAbsolute(path) || resolve(path) !== path) throw new Error("Invalid private path");
  let prefix = parse(path).root;
  for (const component of path.slice(prefix.length).split(sep)) {
    prefix = resolve(prefix, component);
    try { if (lstatSync(prefix).isSymbolicLink()) throw new Error("Private path contains symlink"); }
    catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error; }
  }
  try { mkdirSync(path, { mode: 0o700 }); chmodSync(path, 0o700); }
  catch (error) { if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error; }
  const info = lstatSync(path);
  if (info.uid !== process.getuid?.() || !info.isDirectory() || (info.mode & 0o777) !== 0o700) throw new Error("Private directory must be 0700");
  // The child metadata and its link in the parent must both survive a crash.
  // Repeat on existing paths because a prior creation/fsync may have failed.
  for (const directory of [path, dirname(path)]) {
    const fd = openSync(directory, constants.O_RDONLY);
    try { fsyncSync(fd); } finally { closeSync(fd); }
  }
}
export function readPrivate(path: string, maximum = 65_536, observe?: NativeFileObserver): string {
  let info: ReturnType<typeof lstatSync>;
  try { info = lstatSync(path); }
  catch (error) { observePrivateFailure(observe, { file_step: "lstat" }); throw error; }
  // Preserve the original predicate order and short circuit. Report only the
  // first failed check, never raw metadata or an inference about its cause.
  const check = info.uid !== process.getuid?.() ? "owner" : info.nlink !== 1 ? "links" : !info.isFile() ? "type"
    : (info.mode & 0o777) !== 0o600 ? "mode" : info.size > maximum ? "size" : undefined;
  if (check) {
    const error = new Error("Invalid private file");
    const links = !Number.isSafeInteger(info.nlink) || info.nlink < 0 ? "invalid" : info.nlink === 0 ? "zero" : info.nlink === 1 ? "one" : "multiple";
    observePrivateFailure(observe, { file_step: "metadata", file_check: check, file_links: links });
    throw error;
  }
  try { return readFileSync(path, "utf8"); }
  catch (error) { observePrivateFailure(observe, { file_step: "read", file_links: "one" }); throw error; }
}
export function writePrivate(path: string, value: string, exclusive = false): void {
  const target = exclusive ? path : `${path}.pending`;
  const fd = openSync(target, constants.O_WRONLY | constants.O_CREAT | constants.O_NOFOLLOW | (exclusive ? constants.O_EXCL : constants.O_TRUNC), 0o600);
  try { writeFileSync(fd, value); fsyncSync(fd); } finally { closeSync(fd); }
  if (!exclusive) renameSync(target, path);
  const directory = openSync(dirname(path), constants.O_RDONLY);
  try { fsyncSync(directory); } finally { closeSync(directory); }
}
