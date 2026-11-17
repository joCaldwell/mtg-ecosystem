import { HttpError, ProtocolError, TransportError } from "@multiverse/game-server-sdk";
import type { GameDiagnosticEvent, GameDiagnosticSink } from "@mtg-ecosystem/deck-builder/contracts";

type Event = Extract<GameDiagnosticEvent, { source: "adapter" }>;
export type NativeDiagnosticCause = Event["cause"];
export type NativeFileDetail = Pick<Event, "file_scope" | "file_step" | "file_check" | "file_links">;
export type NativeTimingDetail = Pick<Event, "timing_attempt" | "timing_phase" | "timing_request_ms" | "timing_headers_ms" | "timing_resume_ms" | "timing_file_ms" | "timing_decode_ms" | "timing_compare_ms" | "timing_elapsed_ms">;
export const timingMarks = ["timing_request_ms", "timing_headers_ms", "timing_resume_ms", "timing_file_ms", "timing_decode_ms", "timing_compare_ms", "timing_elapsed_ms"] as const;
const phases = ["fetch", "resume", "identity", "status_file", "status_decode", "compare", "retry_wait"];
/** A failed clock removes timing evidence, never operation results. */
export function monotonicStamp(): number {
  try { return performance.now(); } catch { return NaN; }
}
export function safeTiming(detail: NativeTimingDetail): NativeTimingDetail {
  const safe: NativeTimingDetail = {};
  const { timing_attempt, timing_phase } = detail;
  if (timing_attempt !== undefined && [1, 2, 3, 4].includes(timing_attempt)) safe.timing_attempt = timing_attempt;
  if (timing_phase !== undefined && phases.includes(timing_phase)) safe.timing_phase = timing_phase;
  for (const key of timingMarks) {
    const value = detail[key];
    if (typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= 86_400_000) safe[key] = value;
  }
  return safe;
}
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
  return (game_id: string, stage: Event["stage"], cause: NativeDiagnosticCause, attempt?: Event["attempt"], detail?: NativeFileDetail, timing?: NativeTimingDetail) => {
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
      const marks = stage === "status" && timing ? safeTiming(timing) : {};
      void Promise.resolve(sink?.({ source: "adapter", game_id, stage, cause, ...(attempt === undefined ? {} : { attempt }), ...safe, ...marks })).catch(() => {});
    }
    catch { /* diagnostic failures never participate in lifecycle control */ }
  };
}
