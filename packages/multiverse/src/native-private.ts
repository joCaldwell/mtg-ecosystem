import { constants, openSync, closeSync, fsyncSync, writeFileSync, readFileSync, lstatSync, mkdirSync, chmodSync, renameSync } from "node:fs";
import { dirname, isAbsolute, parse, resolve, sep } from "node:path";
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
}
export function readPrivate(path: string, maximum = 65_536): string {
  const info = lstatSync(path);
  if (info.uid !== process.getuid?.() || info.nlink !== 1 || !info.isFile() || (info.mode & 0o777) !== 0o600 || info.size > maximum) throw new Error("Invalid private file");
  return readFileSync(path, "utf8");
}
export function writePrivate(path: string, value: string, exclusive = false): void {
  const target = exclusive ? path : `${path}.pending`;
  const fd = openSync(target, constants.O_WRONLY | constants.O_CREAT | constants.O_NOFOLLOW | (exclusive ? constants.O_EXCL : constants.O_TRUNC), 0o600);
  try { writeFileSync(fd, value); fsyncSync(fd); } finally { closeSync(fd); }
  if (!exclusive) renameSync(target, path);
  const directory = openSync(dirname(path), constants.O_RDONLY);
  try { fsyncSync(directory); } finally { closeSync(directory); }
}
