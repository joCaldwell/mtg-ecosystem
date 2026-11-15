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
  /** Requires qualified public session composition, every reachable decision,
   * exact journaling/Web Locks, and authenticated same-origin native proxy. */
  browser_session_qualified?: boolean;
  operation_timeout_ms?: number;
  human_think_time_ms?: number;
  game_lifetime_ms?: number;
  supervision_interval_ms?: number;
  now?: () => number;
}
