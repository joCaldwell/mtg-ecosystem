import { setTimeout as delay } from "node:timers/promises";
import { GameClient, parseSeatBootstrap } from "@multiverse/game-server-sdk";
import type { GameRecord, GameIdentity, NativeGameStatus } from "@mtg-ecosystem/deck-builder/contracts";
import { nativeDiagnostics, nativeFailureCause, type NativeDiagnosticCause, type NativeFileDetail } from "./native-diagnostics.ts";
import type { NativeFileObserver } from "./native-private.ts";

/** The adapter's status boundary, separated for synthetic transport/file tests.
 * Reads remain lazy so child liveness is checked before private seat access. */
export async function nativeStatus(game: GameRecord, signal: AbortSignal, input: {
  identity: GameIdentity;
  state?: { child: { exitCode: number | null; signalCode: string | null }; stopping: boolean; origin?: string;
    bots?: { status(): NativeGameStatus["consumers"] } };
  bootstrap(observe?: NativeFileObserver): string;
  readStatus(observe?: NativeFileObserver): string;
  diagnostic: ReturnType<typeof nativeDiagnostics>;
}): Promise<NativeGameStatus> {
  let cause: NativeDiagnosticCause = "state_unavailable";
  let fileDetail: NativeFileDetail | undefined;
  try {
    signal.throwIfAborted();
    const state = input.state;
    if (state && (state.child.exitCode !== null || state.child.signalCode !== null)) { cause = "child_already_exited"; throw new Error("Native lifecycle unavailable"); }
    if (!state || state.stopping || !state.bots) throw new Error("Native lifecycle unavailable");
    cause = "identity_failure";
    if (!["catalog_sha256", "profile", "engine_build_id", "server_build_id"].every(key => game.identity[key as keyof GameIdentity] === input.identity[key as keyof GameIdentity])) throw new Error("Native lifecycle unavailable");
    cause = "private_file";
    const bytes = input.bootstrap(detail => { fileDetail = detail; });
    cause = "schema_failure";
    const seat = parseSeatBootstrap(bytes);
    const sdk = new GameClient({ baseURL: state.origin!, matchId: game.id, capability: seat.capability });
    for (let attempt = 0; attempt < 4; attempt++) {
      cause = "adapter_failure";
      const resume = await sdk.resume({ signal });
      cause = "identity_failure";
      if (resume.viewer !== 0 || resume.engine_build_id !== game.identity.engine_build_id || resume.server_build_id !== game.identity.server_build_id || resume.catalog_sha256 !== game.identity.catalog_sha256) throw new Error("Native lifecycle unavailable");
      cause = "private_file";
      fileDetail = { file_scope: "status" };
      const statusBytes = input.readStatus(detail => { fileDetail = { ...detail, file_scope: "status" }; });
      cause = "status_decode";
      const row = JSON.parse(statusBytes) as { version: number; human_waiting_since: number | null; human_decision_token: string | null };
      if (row.version !== 1 || (row.human_waiting_since !== null && (!Number.isSafeInteger(row.human_waiting_since) || row.human_waiting_since < 0))) throw new Error("Native lifecycle unavailable");
      const token = resume.state.status === "available" ? resume.state.observation.decision?.token ?? null : null;
      // Timestamp is still bound to the exact human decision. Diagnostics do
      // not change the four attempts, 100ms waits or final fail-closed rejection.
      if (row.human_decision_token === token) return { identity: { ...input.identity }, consumers: state.bots.status(), human_waiting_since: row.human_waiting_since, outcome: resume.outcome ? resume.outcome.outcome === "win" ? { kind: "win", winner: resume.outcome.player as 0 | 1 | 2 | 3 } : { kind: "draw" } : null };
      input.diagnostic(game.id, "status", "token_mismatch", (attempt + 1) as 1 | 2 | 3 | 4);
      await delay(100, undefined, { signal });
    }
    cause = "token_mismatch_exhausted";
    throw new Error("Native lifecycle unavailable");
  } catch (error) {
    input.diagnostic(game.id, "status", nativeFailureCause(error, signal, cause), undefined, fileDetail);
    throw error;
  }
}
