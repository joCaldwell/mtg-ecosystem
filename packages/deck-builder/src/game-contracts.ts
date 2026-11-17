/** Product coordination DTOs, not replacements for native rules/SDK DTOs. */
export interface GameDeck {
  deck_id: number;
  revision: number;
  cards: ReadonlyArray<{ oracle_id: string; role: "card" | "commander" | "companion"; quantity: number }>;
  sha256: string;
}
export interface GameIdentity {
  catalog_sha256: string;
  profile: string;
  engine_build_id: string;
  server_build_id: string;
}
export type GameAdmission =
  | { state: "not_checked" | "checking" }
  | { state: "stale_revision"; current_revision: number }
  | { state: "unavailable"; reason: "native_unconfigured" | "native_unavailable" | "identity_mismatch" | "session_incomplete" }
  | { state: "unsupported"; code: "deck" | "multiple_commanders" | "companion" | "catalog" }
  | { state: "supported"; identity: GameIdentity; deck_sha256: string; launch_ready: boolean };
export type GameOutcome = { kind: "win"; winner: 0 | 1 | 2 | 3 } | { kind: "draw" };
export type GamePause = "consumer_failed" | "human_timeout" | "native_unavailable" | "identity_mismatch" | "lifetime_exceeded";
export interface GameRecord {
  id: string;
  deck: GameDeck;
  identity: GameIdentity;
  state: "provisioning" | "active" | "paused" | "completed" | "interrupted";
  pause: GamePause | null;
  outcome: GameOutcome | null;
  created_at: number;
  updated_at: number;
  human_waiting_since: number | null;
  /** Durable acknowledgment that every automated consumer has stopped. */
  consumers_stopped: boolean;
}
export interface GameLaunchResult {
  admission: GameAdmission;
  game?: GameRecord;
}
/** Minimal native stop identity survives account deletion until acknowledgment.
 * It contains no account, deck, source, notes or seat credential. */
export type GameStopTarget = Pick<GameRecord, "id" | "identity">;
/** Each bot owns one native seat consumer. No credential is returned by status. */
export interface NativeGameStatus {
  identity: GameIdentity;
  consumers: readonly ["running" | "failed" | "stopped", "running" | "failed" | "stopped", "running" | "failed" | "stopped"];
  /** Stable across native recovery for the current human decision; a new
   * decision starts a new wait, polling/heartbeats never reset it. */
  human_waiting_since: number | null;
  outcome: GameOutcome | null;
}
/** Only reviewed human-seat HTTP. Authorization stays in memory. Body retains
 * exact original UTF-8 bytes through the proxy; never parse/re-serialize it. */
export interface NativeGameProxyRequest {
  method: "GET" | "POST";
  path: string;
  body: string;
  authorization: string;
}
export interface NativeGameProxyResponse { status: number; body: string }
/** Monotonic offsets from one attempt's start, saturated at 24 hours. Missing
 * marks mean the boundary was not reached (or its clock was unavailable). */
export interface GameStatusTiming {
  timing_attempt?: 1 | 2 | 3 | 4;
  timing_phase?: "fetch" | "resume" | "identity" | "status_file" | "status_decode" | "compare" | "retry_wait";
  timing_request_ms?: number;
  timing_headers_ms?: number;
  timing_resume_ms?: number;
  timing_file_ms?: number;
  timing_decode_ms?: number;
  timing_compare_ms?: number;
  timing_elapsed_ms?: number;
}
/** Host-private observations only: never append errors, protocol data or credentials.
 * Sinks must be bounded and nonblocking; returned promises are never awaited. */
export type GameDiagnosticEvent =
  | { source: "coordinator"; game_id: string; stage: "ensure" | "status" | "stop";
      phase: "operation" | "cleanup" | "shutdown" | "owner_deleted";
      cause: "started" | "succeeded" | "deadline" | "adapter_failure" | "identity_failure" | "status_invalid";
      /** Actual timer fire minus its monotonic arm + delay; bounded to 24h. */
      deadline_lateness_ms?: number }
  | (GameStatusTiming & { source: "adapter"; game_id: string; stage: "ensure" | "status" | "stop" | "ensure_cleanup" | "child";
      cause: "aborted" | "identity_failure" | "state_unavailable" | "child_already_exited" | "private_file" | "status_decode"
        | "http_failure" | "schema_failure" | "transport_failure" | "adapter_failure" | "startup_protocol" | "startup_failure"
        | "presentation_failure" | "bots_failure" | "token_mismatch" | "token_mismatch_exhausted"
        | "cleanup_started" | "cleanup_succeeded" | "cleanup_failed" | "child_exit" | "child_exit_during_stop" | "child_error" | "status_timing";
      attempt?: 1 | 2 | 3 | 4;
      /** Optional closed private-file observations; absence retains older sinks. */
      file_scope?: "bootstrap_guard" | "bootstrap_seat" | "status";
      file_step?: "guard" | "lstat" | "metadata" | "read";
      file_check?: "identity" | "lifecycle" | "owner" | "links" | "type" | "mode" | "size";
      file_links?: "zero" | "one" | "multiple" | "invalid" });
export type GameDiagnosticSink = (event: GameDiagnosticEvent) => void | Promise<void>;
export interface NativeGameAdapter {
  /** Exact identities are mandatory on admission AND recovery. */
  identity: GameIdentity;
  proxy?(game: GameRecord, request: NativeGameProxyRequest, signal: AbortSignal): Promise<NativeGameProxyResponse>;
  /** True only after real durable create/recover + isolated bots + stop proof. */
  lifecycle_qualified: boolean;
  admit(deck: GameDeck, signal: AbortSignal): Promise<GameAdmission>;
  /** Must durably deduplicate by game.id, including uncertain create outcomes.
   * Bind all four immutable decks, exact identities and human seat 0. Bot
   * credentials, journals and distinct consumer leases stay in private storage.
   * Retrying ensure must recover the SAME match; never create a replacement. */
  ensure(game: GameRecord, signal: AbortSignal): Promise<void>;
  status(game: GameRecord, signal: AbortSignal): Promise<NativeGameStatus>;
  /** Fence in-flight ensure calls, pause host execution and join all consumers.
   * Must stop even after ensure timed out with unknown outcome. Abort cannot
   * permit a late operation to spawn unowned consumers. The bridge must also
   * stop consumers if this coordinator process dies. Never invent rules loss. */
  stop(game: GameStopTarget, reason: GamePause | "completed" | "shutdown" | "owner_deleted", signal: AbortSignal): Promise<void>;
  /** Return ONLY generated SeatBootstrap file bytes for human seat 0, <=4096
   * bytes, bound to exact match/build/catalog and configured same-origin proxy.
   * Caller must validate with the generated SDK before delivery. Never return
   * operator/bot capabilities, arbitrary JSON or a ClientConfig as seat auth. */
  humanBootstrap(game: GameRecord, signal: AbortSignal): Promise<string>;
  /** Use the generated native SDK parser, then verify viewer=0, match_id=game.id,
   * exact engine/server/catalog pins and configured proxy origin. Must return
   * false for extra credentials or an unqualified delivery path. */
  validateHumanBootstrap(bytes: string, game: GameRecord): boolean;
}
export interface GameCoordinatorOptions {
  native?: NativeGameAdapter;
  /** Optional private diagnostics; failures must never affect game operations. */
  diagnostic?: GameDiagnosticSink;
  /** Requires qualified public session composition, every reachable decision,
   * exact journaling/Web Locks, and authenticated same-origin native proxy. */
  browser_session_qualified?: boolean;
  operation_timeout_ms?: number;
  human_think_time_ms?: number;
  game_lifetime_ms?: number;
  supervision_interval_ms?: number;
  now?: () => number;
}
