import { randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

/** An exclusive offline-operation/server lease, also respected by restore. */
export function acquireProcessLock(path: string): () => void {
  mkdirSync(dirname(path), { recursive: true });
  const token = randomUUID();
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      writeFileSync(path, JSON.stringify({ pid: process.pid, token }), { flag: "wx", mode: 0o600 });
      return () => {
        if (existsSync(path) && JSON.parse(readFileSync(path, "utf8")).token === token) rmSync(path);
      };
    } catch (e: any) {
      if (e.code !== "EEXIST") throw e;
      let pid: number;
      try { pid = JSON.parse(readFileSync(path, "utf8")).pid; }
      catch { throw new Error(`Cannot read process lock ${path}; inspect it before retrying`); }
      if (!Number.isInteger(pid) || pid <= 0) throw new Error(`Invalid process lock ${path}`);
      try { process.kill(pid, 0); }
      catch (error: any) {
        if (error.code === "ESRCH") { rmSync(path); continue; }
        throw error;
      }
      throw new Error("The server or another offline operation is running; stop it before restoring");
    }
  }
  throw new Error("Could not acquire process lock");
}
