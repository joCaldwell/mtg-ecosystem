import { spawn, type ChildProcess } from "node:child_process";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { existsSync } from "node:fs";
import { privateDirectory, writePrivate } from "./native-private.ts";
export interface BotGroup {
  status(): readonly ["running" | "failed" | "stopped", "running" | "failed" | "stopped", "running" | "failed" | "stopped"];
  stop(): Promise<void>;
}
/** Three distinct OS processes and kernel flock leases. Each receives only its
 * own seat file, journal and pipe lifetime; no operator credential or other seat. */
export async function startBots(root: string, session: string, profile: string, logicalOrigin: string, signal: AbortSignal): Promise<BotGroup> {
  const children: ChildProcess[] = [];
  const joined: Promise<void>[] = [];
  let stopping = false, stopped: Promise<void> | undefined;
  const group: BotGroup = {
    status: () => children.map(child => stopping ? "stopped" : child.exitCode === null && child.signalCode === null ? "running" : "failed") as unknown as ReturnType<BotGroup["status"]>,
    stop: () => stopped ??= (async () => {
      stopping = true;
      for (const child of children) child.stdin?.end();
      // Parent EOF is primary lifetime. SIGKILL bounds cleanup after a broken
      // runtime; the flock wrapper holds the lease until its child has exited.
      const timer = setTimeout(() => { for (const child of children) child.kill("SIGKILL"); }, 2000);
      try { await Promise.all(joined); } finally { clearTimeout(timer); }
    })(),
  };
  try {
    for (const seat of [1, 2, 3]) {
      signal.throwIfAborted();
      const directory = join(root, `bot-${seat}`); privateDirectory(directory);
      const lock = join(directory, "lease"); if (!existsSync(lock)) writePrivate(lock, "", true);
      const child = spawn("flock", ["--exclusive", "--nonblock", "--no-fork", lock, process.execPath, fileURLToPath(new URL("./native-bot-worker.ts", import.meta.url)), directory, join(root, session, `seat-${seat}.json`), profile, logicalOrigin], {
        stdio: ["pipe", "pipe", "ignore"], env: { PATH: "/usr/bin:/bin", ...(process.env.LD_LIBRARY_PATH ? { LD_LIBRARY_PATH: process.env.LD_LIBRARY_PATH } : {}) },
      });
      child.stdin!.on("error", () => {});
      children.push(child);
      joined.push(new Promise(resolve => { child.once("close", () => resolve()); child.once("error", () => resolve()); }));
      await new Promise<void>((resolve, reject) => {
        let output = "";
        const abort = () => reject(new Error("Automated startup cancelled"));
        signal.addEventListener("abort", abort, { once: true });
        const cleanup = () => signal.removeEventListener("abort", abort);
        child.stdout!.on("data", chunk => { output += chunk.toString(); if (output === '{"ready":true}\n') { cleanup(); resolve(); } else if (output.length > 128) { cleanup(); reject(new Error("Invalid automated startup")); } });
        child.once("exit", () => { cleanup(); reject(new Error("Automated consumer unavailable")); });
        child.once("error", () => { cleanup(); reject(new Error("Automated consumer unavailable")); });
      });
    }
    return group;
  } catch (error) { await group.stop(); throw error; }
}
