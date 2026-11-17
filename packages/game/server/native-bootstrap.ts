import { join } from "node:path";
import type { GameIdentity } from "../shared/contracts.ts";
import { readPrivate, observePrivateFailure, type NativeFileObserver } from "./native-private.ts";

/** Lazy lifecycle access preserves identity -> state -> root -> seat ordering. */
export function readNativeBootstrap(identity: GameIdentity, expected: GameIdentity,
  state: () => { session?: string; stopping: boolean; child: { exitCode: number | null; signalCode: string | null } } | undefined,
  root: () => string, observe?: NativeFileObserver): string {
  let check: "identity" | "lifecycle" = "identity";
  let path: string;
  try {
    if (!["catalog_sha256", "profile", "engine_build_id", "server_build_id"].every(key => identity[key as keyof GameIdentity] === expected[key as keyof GameIdentity])) throw new Error("Native lifecycle unavailable");
    check = "lifecycle";
    const current = state();
    if (!current?.session || current.stopping || current.child.exitCode !== null || current.child.signalCode !== null) throw new Error("Native lifecycle unavailable");
    check = "identity";
    path = join(root(), current.session, "seat-0.json");
  } catch (error) {
    observePrivateFailure(observe, { file_scope: "bootstrap_guard", file_step: "guard", file_check: check });
    throw error;
  }
  return readPrivate(path, 4096, detail => observePrivateFailure(observe, { file_scope: "bootstrap_seat", ...detail }));
}
