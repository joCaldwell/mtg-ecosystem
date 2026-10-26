import { randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { hostname } from "node:os";
import { resolve } from "node:path";

const held = new Set<string>();

/** An exclusive offline-operation/server lease, also respected by restore. */
export function acquireProcessLock(path: string): () => void {
  path = resolve(path);
  mkdirSync(dirname(path), { recursive: true });
  const token = randomUUID();
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      writeFileSync(path, JSON.stringify({ pid: process.pid, hostname: hostname(), token }), { flag: "wx", mode: 0o600 });
      held.add(path);
      return () => {
        if (existsSync(path) && JSON.parse(readFileSync(path, "utf8")).token === token) rmSync(path);
        held.delete(path);
      };
    } catch (e: any) {
      if (e.code !== "EEXIST") throw e;
      let pid: number, host: string | undefined;
      try { ({ pid, hostname: host } = JSON.parse(readFileSync(path, "utf8"))); }
      catch { throw new Error(`Cannot read process lock ${path}; inspect it before retrying`); }
      if (!Number.isInteger(pid) || pid <= 0) throw new Error(`Invalid process lock ${path}`);
      // A volume belongs to one container. Container PID namespaces reuse 1;
      // a file referring to this new PID (or another container) is stale.
      // In-process acquisitions still exclude restore against our own server.
      if (!held.has(path) && (pid === process.pid || (host !== undefined && host !== hostname()))) {
        rmSync(path); continue;
      }
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
