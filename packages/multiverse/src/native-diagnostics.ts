import { HttpError, ProtocolError, TransportError } from "@multiverse/game-server-sdk";
import type { GameDiagnosticEvent, GameDiagnosticSink } from "@mtg-ecosystem/deck-builder/contracts";

type Event = Extract<GameDiagnosticEvent, { source: "adapter" }>;
export type NativeDiagnosticCause = Event["cause"];
/** Only locally chosen codes cross this boundary. Never inspect an error's
 * message/name/body/stack (including accessors on an arbitrary thrown value). */
export function nativeFailureCause(error: unknown, signal: AbortSignal, fallback: NativeDiagnosticCause): NativeDiagnosticCause {
  if (signal.aborted) return "aborted";
  if (error instanceof HttpError) return "http_failure";
  if (error instanceof ProtocolError) return "schema_failure";
  if (error instanceof TransportError) return "transport_failure";
  return fallback;
}
export function nativeDiagnostics(sink?: GameDiagnosticSink) {
  return (game_id: string, stage: Event["stage"], cause: NativeDiagnosticCause, attempt?: Event["attempt"]) => {
    try { void Promise.resolve(sink?.({ source: "adapter", game_id, stage, cause, ...(attempt === undefined ? {} : { attempt }) })).catch(() => {}); }
    catch { /* diagnostic failures never participate in lifecycle control */ }
  };
}
