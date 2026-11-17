import { HttpError, ProtocolError, TransportError } from "@multiverse/game-server-sdk";
import type { GameDiagnosticEvent, GameDiagnosticSink } from "@mtg-ecosystem/deck-builder/contracts";

type Event = Extract<GameDiagnosticEvent, { source: "adapter" }>;
export type NativeDiagnosticCause = Event["cause"];
export type NativeFileDetail = Pick<Event, "file_scope" | "file_step" | "file_check" | "file_links">;
/** Only locally chosen codes cross this boundary. Never inspect an error's
 * message/name/body/stack (including accessors on an arbitrary thrown value). */
export function nativeFailureCause(error: unknown, signal: AbortSignal, fallback: NativeDiagnosticCause): NativeDiagnosticCause {
  try {
    if (signal.aborted) return "aborted";
    if (error instanceof HttpError) return "http_failure";
    if (error instanceof ProtocolError) return "schema_failure";
    if (error instanceof TransportError) return "transport_failure";
  } catch { /* even hostile exception objects cannot break observation */ }
  return fallback;
}
export function nativeDiagnostics(sink?: GameDiagnosticSink) {
  return (game_id: string, stage: Event["stage"], cause: NativeDiagnosticCause, attempt?: Event["attempt"], detail?: NativeFileDetail) => {
    if (!/^[a-f0-9]{32}$/.test(game_id)) return;
    try {
      // Reconstruct detail rather than spreading a caller's object into a sink.
      const safe: NativeFileDetail = {};
      if (cause === "private_file" && detail) {
        const { file_scope, file_step, file_check, file_links } = detail;
        if (file_scope !== undefined && ["bootstrap_guard", "bootstrap_seat", "status"].includes(file_scope)) safe.file_scope = file_scope;
        if (file_step !== undefined && ["guard", "lstat", "metadata", "read"].includes(file_step)) safe.file_step = file_step;
        if (file_check !== undefined && ["identity", "lifecycle", "owner", "links", "type", "mode", "size"].includes(file_check)) safe.file_check = file_check;
        if (file_links !== undefined && ["zero", "one", "multiple", "invalid"].includes(file_links)) safe.file_links = file_links;
      }
      void Promise.resolve(sink?.({ source: "adapter", game_id, stage, cause, ...(attempt === undefined ? {} : { attempt }), ...safe })).catch(() => {});
    }
    catch { /* diagnostic failures never participate in lifecycle control */ }
  };
}
