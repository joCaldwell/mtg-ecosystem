import type { ApplicationFactory } from "./application.ts";
import { join } from "node:path";
import { createHostedAuth } from "./auth.ts";
import { createApp } from "./app.ts";
import { DATA_DIR, DEFAULT_DB_PATH, openDb } from "./db.ts";
import { createJobs } from "./jobs.ts";
import { acquireProcessLock } from "./process-lock.ts";
import { migrateLegacyToken } from "./tokens.ts";
import { getMode, type Mode } from "./mode.ts";

export interface HostOptions {
  /** Product-owned static bundle; the backend never imports the product app. */
  webDist?: string;
  isApplicationPath?: (pathname: string) => boolean;
  port?: number;
  mode?: Mode;
  /** Explicit paths support disposable integration fixtures and operators. */
  databasePath?: string;
  dataDirectory?: string;
  backgroundJobs?: boolean;
  application?: ApplicationFactory;
}

/** Public process composition boundary. Accounts, tenancy, database migrations,
 * API and MCP remain one capability service; the product owns its frontend and
 * entrypoint. Construction does not listen or install process signal handlers. */
export async function createHost(options: HostOptions = {}) {
  const mode = options.mode ?? getMode();
  const port = options.port ?? Number(process.env.DECKBUILDER_PORT ?? 8787);
  if (!Number.isInteger(port) || port < 0 || port > 65535) throw new Error("Invalid application port");
  const databasePath = options.databasePath ?? DEFAULT_DB_PATH;
  const dataDirectory = options.dataDirectory ?? DATA_DIR;
  const release = acquireProcessLock(`${databasePath}.server-lock`);
  let db: ReturnType<typeof openDb> | undefined;
  let app: ReturnType<typeof createApp> | undefined;
  let jobs: ReturnType<typeof createJobs> | undefined;
  try {
    db = openDb(databasePath);
    if (mode === "local") migrateLegacyToken(db, join(dataDirectory, "agent-token"));
    if (options.backgroundJobs !== false) jobs = createJobs(db, {
      dbPath: databasePath, dataDir: dataDirectory,
      onChange: () => app?.notifyAll({ kind: "jobs" }),
      ...(mode === "hosted" ? { writeFailure: (write: () => void) => app!.withDatabase(write) } : {}),
    });
    const auth = mode === "hosted" ? createHostedAuth(db) : undefined;
    await auth?.ready;
    app = createApp(db, { mode, port, jobs, auth, webDist: options.webDist, isApplicationPath: options.isApplicationPath, application: options.application });
  } catch (error) {
    try { await jobs?.close(); }
    finally { db?.close(); release(); }
    throw error;
  }
  const application = app!;
  const database = db!;
  let closing: Promise<void> | undefined;
  let starting: Promise<{ port: number; mode: Mode }> | undefined;
  let cancelStart: (() => void) | undefined;
  const close = (): Promise<void> => closing ??= (async () => {
    // Closing a listener before its listening event prevents the callback from
    // firing. Settle its caller explicitly rather than leaving start pending.
    cancelStart?.();
    try { await application.close(); }
    finally { try { await jobs?.close(); } finally { database.close(); release(); } }
  })();
  const start = (): Promise<{ port: number; mode: Mode }> => {
    if (closing) return Promise.reject(new Error("Application host is closed"));
    return starting ??= new Promise<{ port: number; mode: Mode }>((resolve, reject) => {
      const fail = (error: Error) => {
        cancelStart = undefined;
        void close().then(() => reject(error), reject);
      };
      cancelStart = () => {
        cancelStart = undefined;
        // A bind error can already be queued when close wins the race. Keep
        // the once-only handler so cancellation cannot turn it into an
        // unhandled server error; the caller is already definitively closed.
        reject(new Error("Application host is closed"));
      };
      application.server.once("error", fail);
      application.server.listen(port, mode === "local" ? "127.0.0.1" : "0.0.0.0", () => {
        application.server.removeListener("error", fail);
        cancelStart = undefined;
        if (closing) { reject(new Error("Application host is closed")); return; }
        const address = application.server.address();
        if (!address || typeof address === "string") { void close().then(() => reject(new Error("Application listener unavailable"))); return; }
        jobs?.start();
        application.startApplication();
        resolve({ port: address.port, mode });
      });
    });
  };
  return { server: application.server, start, close, paths: { database: databasePath, data: dataDirectory } };
}
