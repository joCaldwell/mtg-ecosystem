import { createHash, randomBytes } from "node:crypto";
import type { Database } from "./sqlite.ts";
import type { Actor } from "./actor.ts";
import { requireDeckOwner, requireUi } from "./actor.ts";
import { accountInfo } from "./accounts.ts";
import { databaseGate } from "./auth-database.ts";
import { withTransaction } from "./db.ts";
import { ServiceError } from "./errors.ts";
import type { GameAdmission, GameCoordinatorOptions, GameDeck, GameIdentity, GameLaunchResult, GamePause, GameRecord, GameStopTarget, NativeGameStatus } from "./game-contracts.ts";

const hash = (value: unknown) => createHash("sha256").update(JSON.stringify(value)).digest("hex");
const sameIdentity = (a: GameIdentity, b: GameIdentity) => a.catalog_sha256 === b.catalog_sha256 && a.profile === b.profile && a.engine_build_id === b.engine_build_id && a.server_build_id === b.server_build_id;
const publicIdentity = (i: GameIdentity): GameIdentity => ({ catalog_sha256: i.catalog_sha256, profile: i.profile, engine_build_id: i.engine_build_id, server_build_id: i.server_build_id });
const validIdentity = (i: GameIdentity) => i && /^[a-f0-9]{64}$/.test(i.catalog_sha256) && [i.profile, i.engine_build_id, i.server_build_id].every(v => typeof v === "string" && v.length > 0 && v.length <= 256);
const unavailable = (reason: Extract<GameAdmission, { state: "unavailable" }>["reason"]): GameAdmission => ({ state: "unavailable", reason });

/** One coordinator per account DB/process lease. SQL reservations never span
 * native I/O. Account/deck authority is rechecked inside each SQL reservation. */
export function createGameCoordinator(db: Database, options: GameCoordinatorOptions = {}) {
  const native = options.native, gate = databaseGate(db), now = options.now ?? Date.now;
  const timeout = options.operation_timeout_ms ?? 10_000;
  const thinkTime = options.human_think_time_ms ?? 30 * 60_000;
  const lifetime = options.game_lifetime_ms ?? 8 * 60 * 60_000;
  const interval = options.supervision_interval_ms ?? 5_000;
  for (const n of [timeout, thinkTime, lifetime, interval]) if (!Number.isSafeInteger(n) || n < 1) throw new Error("Invalid game coordinator limits");
  let admissionCalls = 0;
  let closed = false, timer: ReturnType<typeof setInterval> | undefined, ticking: Promise<void> | undefined;
  const inflight = new Map<string, Promise<GameRecord>>();
  const owned = new Map<string, GameRecord>();
  const sql = <T>(work: () => T) => gate.run(work);
  const authorize = (actor: Actor) => { requireUi(actor, "launch or resume games"); accountInfo(db, actor); };
  function capture(actor: Actor, deckId: number, revision: number): GameDeck | Extract<GameAdmission, { state: "stale_revision" }> {
    authorize(actor);
    if (!Number.isSafeInteger(deckId) || deckId < 1 || !Number.isSafeInteger(revision) || revision < 0) throw new ServiceError("Deck and revision must be valid integers");
    const deck = requireDeckOwner(db, actor, deckId);
    if (deck.revision !== revision) return { state: "stale_revision", current_revision: deck.revision };
    const cards = db.prepare("SELECT oracle_id, role, quantity FROM deck_cards WHERE deck_id=? AND maybe=0 ORDER BY oracle_id").all(deckId) as unknown as GameDeck["cards"];
    const body = { deck_id: deckId, revision, cards };
    return { ...body, sha256: hash(body) };
  }
  const read = (actor: Actor, id: string): GameRecord => {
    authorize(actor);
    const row = db.prepare("SELECT record_json FROM game_launches WHERE id=? AND user_id=?").get(id, actor.user_id) as { record_json: string } | undefined;
    if (!row) throw new ServiceError("Game not found", 404);
    return JSON.parse(row.record_json);
  };
  async function call<T>(work: (signal: AbortSignal) => Promise<T>): Promise<T> {
    const controller = new AbortController();
    let handle: ReturnType<typeof setTimeout> | undefined;
    try {
      return await Promise.race([work(controller.signal), new Promise<never>((_, reject) => {
        handle = setTimeout(() => { controller.abort(); reject(new Error("Native game operation unavailable")); }, timeout);
      })]);
    } finally { clearTimeout(handle); }
  }
  async function check(deck: GameDeck): Promise<GameAdmission> {
    if (!native) return unavailable("native_unconfigured");
    if (!validIdentity(native.identity)) return unavailable("identity_mismatch");
    if (admissionCalls >= 4) return unavailable("native_unavailable");
    admissionCalls++;
    try {
      const answer = await call(signal => native.admit(structuredClone(deck), signal));
      if (answer.state === "supported") {
        if (!validIdentity(answer.identity) || !sameIdentity(answer.identity, native.identity) || answer.deck_sha256 !== deck.sha256) return unavailable("identity_mismatch");
        // Construct an allowlist: adapter fields and credentials never enter JSON.
        return { state: "supported", identity: publicIdentity(native.identity), deck_sha256: deck.sha256, launch_ready: native.lifecycle_qualified === true && options.browser_session_qualified === true };
      }
      if (answer.state === "unsupported" && ["deck", "multiple_commanders", "companion", "catalog"].includes(answer.code)) return { state: "unsupported", code: answer.code };
      if (answer.state === "unavailable" && answer.reason === "identity_mismatch") return unavailable("identity_mismatch");
      return unavailable("native_unavailable");
    } catch { return unavailable("native_unavailable"); }
    finally { admissionCalls--; }
  }
  async function admission(actor: Actor, deckId: number, revision: number): Promise<GameAdmission> {
    const deck = await sql(() => withTransaction(db, () => capture(actor, deckId, revision)));
    if ("state" in deck) return deck;
    const result = await check(deck);
    return sql(() => {
      const latest = capture(actor, deckId, revision);
      return "state" in latest ? latest : result;
    });
  }
  function writeRecord(record: GameRecord): boolean {
    record.updated_at = now();
    return db.prepare("UPDATE game_launches SET record_json=? WHERE id=?").run(JSON.stringify(record), record.id).changes > 0;
  }
  const save = (record: GameRecord): Promise<boolean> => sql(() => writeRecord(record));
  async function pause(record: GameRecord, reason: GamePause | "completed") {
    // Persist intent before stop. Recovery keeps trying stop if its acknowledgment
    // was lost. A supervision fault has no invented rules outcome.
    if (reason !== "completed") { record.state = reason === "human_timeout" || reason === "lifetime_exceeded" ? "interrupted" : "paused"; record.pause = reason; }
    await save(record);
    await call(signal => native!.stop(structuredClone(record), reason, signal));
    record.consumers_stopped = true;
    await save(record);
    owned.delete(record.id);
  }
  function statusValid(status: NativeGameStatus): boolean {
    return !!status && validIdentity(status.identity) && Array.isArray(status.consumers) && status.consumers.length === 3 && status.consumers.every(v => ["running", "failed", "stopped"].includes(v)) &&
      (status.human_waiting_since === null || Number.isSafeInteger(status.human_waiting_since) && status.human_waiting_since >= 0 && status.human_waiting_since <= now()) &&
      (status.outcome === null || status.outcome.kind === "draw" || status.outcome.kind === "win" && [0, 1, 2, 3].includes(status.outcome.winner));
  }
  async function reconcile(record: GameRecord): Promise<GameRecord> {
    if (!native?.lifecycle_qualified || closed) return record;
    // Recovery must retain unacknowledged stop ownership before every state
    // branch, including already paused/completed/interrupted records.
    if (!record.consumers_stopped) owned.set(record.id, record);
    try {
      if (!sameIdentity(record.identity, native.identity)) { owned.set(record.id, record); await pause(record, "identity_mismatch"); return record; }
      if (record.state === "paused" || record.state === "completed" || record.state === "interrupted") {
        if (record.consumers_stopped) return record;
        await pause(record, record.state === "completed" ? "completed" : record.pause!); return record;
      }
      record.consumers_stopped = false;
      owned.set(record.id, record);
      if (now() - record.created_at >= lifetime) { await pause(record, "lifetime_exceeded"); return record; }
      // Reserve durable cleanup ownership before any operation that may start
      // consumers. A concurrent account deletion must observe this false flag
      // and atomically retain its minimal native stop identity.
      if (!await save(record)) { owned.delete(record.id); return record; }
      await call(signal => native.ensure(structuredClone(record), signal));
      const status = await call(signal => native.status(structuredClone(record), signal));
      if (!statusValid(status) || !sameIdentity(record.identity, status.identity)) { await pause(record, "identity_mismatch"); return record; }
      if (status.outcome) {
        record.outcome = status.outcome.kind === "draw" ? { kind: "draw" } : { kind: "win", winner: status.outcome.winner };
        record.state = "completed"; record.pause = null;
        await pause(record, "completed"); return record;
      }
      if (status.consumers.some(v => v !== "running")) { await pause(record, "consumer_failed"); return record; }
      record.human_waiting_since = status.human_waiting_since;
      if (record.human_waiting_since !== null && now() - record.human_waiting_since >= thinkTime) { await pause(record, "human_timeout"); return record; }
      record.state = "active"; record.pause = null;
      if (!await save(record)) { await call(signal => native.stop(record, "owner_deleted", signal)); owned.delete(record.id); }
    } catch {
      if (record.state !== "completed" && record.state !== "interrupted") { record.state = "paused"; record.pause = "native_unavailable"; await save(record); }
      // Do not release the consumer lease on failed stop; next sweep retries.
      try { await call(signal => native.stop(record, record.state === "completed" ? "completed" : "native_unavailable", signal)); record.consumers_stopped = true; await save(record); owned.delete(record.id); } catch { /* retained for supervision retry */ }
    }
    return record;
  }
  function run(record: GameRecord): Promise<GameRecord> {
    const existing = inflight.get(record.id); if (existing) return existing;
    // Tick/admission may have awaited other work since their initial read.
    // Serialize by game and re-read current durable state before applying any
    // lifecycle transition; a stale snapshot must never erase a rules outcome.
    const task = (async () => {
      const row = await sql(() => db.prepare("SELECT record_json FROM game_launches WHERE id=?").get(record.id) as { record_json: string } | undefined);
      return row ? reconcile(JSON.parse(row.record_json) as GameRecord) : record;
    })().finally(() => inflight.delete(record.id));
    inflight.set(record.id, task); return task;
  }
  async function launch(actor: Actor, deckId: number, revision: number, requestId: string): Promise<GameLaunchResult> {
    if (closed) return { admission: unavailable("native_unavailable") };
    if (typeof requestId !== "string" || !/^[A-Za-z0-9_-]{16,128}$/.test(requestId)) throw new ServiceError("Use a stable launch request ID (16–128 letters, numbers, _ or -)");
    const inputHash = hash([deckId, revision]);
    const previous = await sql(() => {
      authorize(actor);
      const row = db.prepare("SELECT record_json,input_hash FROM game_launches WHERE user_id=? AND request_id=?").get(actor.user_id, requestId) as { record_json: string; input_hash: string } | undefined;
      if (row && row.input_hash !== inputHash) throw new ServiceError("Launch request ID already used with different input", 409);
      return row ? JSON.parse(row.record_json) as GameRecord : null;
    });
    if (previous) return resume(actor, previous.id);
    const deck = await sql(() => withTransaction(db, () => capture(actor, deckId, revision)));
    if ("state" in deck) return { admission: deck };
    const admitted = await check(deck);
    // Recheck owner/revision even if native reported unavailable/unsupported.
    const captured = await sql(() => withTransaction(db, () => {
      const latest = capture(actor, deckId, revision);
      if ("state" in latest) return latest;
      if (admitted.state !== "supported" || !admitted.launch_ready) return null;
      const existing = db.prepare("SELECT record_json,input_hash FROM game_launches WHERE user_id=? AND request_id=?").get(actor.user_id, requestId) as { record_json: string; input_hash: string } | undefined;
      if (existing) {
        if (existing.input_hash !== inputHash) throw new ServiceError("Launch request ID already used with different input", 409);
        return JSON.parse(existing.record_json) as GameRecord;
      }
      // No more than one live or uncertain match per owner. A failed create
      // must be reconciled with its exact key, never replaced by a new match.
      const busy = (db.prepare("SELECT record_json FROM game_launches WHERE user_id=?").all(actor.user_id) as { record_json: string }[]).some(r => { const game = JSON.parse(r.record_json) as GameRecord; return !["completed", "interrupted"].includes(game.state) || !game.consumers_stopped; });
      if (busy) throw new ServiceError("Resume your existing game before starting another", 409);
      const record: GameRecord = { id: randomBytes(16).toString("hex"), deck: latest, identity: admitted.identity, state: "provisioning", pause: null, outcome: null, created_at: now(), updated_at: now(), human_waiting_since: null, consumers_stopped: false };
      db.prepare("INSERT INTO game_launches(id,user_id,request_id,input_hash,record_json) VALUES(?,?,?,?,?)").run(record.id, actor.user_id, requestId, inputHash, JSON.stringify(record));
      return record;
    }));
    if (captured && "current_revision" in captured) return { admission: captured };
    if (!captured) return { admission: admitted };
    const game = await run(captured);
    await sql(() => read(actor, game.id));
    return { admission: admitted, game };
  }
  async function resume(actor: Actor, id: string): Promise<GameLaunchResult> {
    const initial = await sql(() => read(actor, id));
    if (closed || !native?.lifecycle_qualified || !options.browser_session_qualified) return { admission: unavailable("session_incomplete"), game: initial };
    // Reconcile the immutable game deck, never the mutable library deck.
    const admitted = await check(initial.deck);
    const game = await sql(() => withTransaction(db, () => {
      const current = read(actor, id);
      if (admitted.state === "supported" && sameIdentity(current.identity, admitted.identity) && current.state === "paused" && current.pause !== "human_timeout" && current.pause !== "lifetime_exceeded") {
        current.state = "provisioning"; current.pause = null; current.consumers_stopped = false;
        if (!writeRecord(current)) throw new ServiceError("Game not found", 404);
      }
      return current;
    }));
    if (admitted.state !== "supported" || !sameIdentity(game.identity, admitted.identity)) return { admission: unavailable("identity_mismatch"), game };
    const result = await run(game);
    await sql(() => read(actor, id));
    return { admission: admitted, game: result };
  }
  async function drainStopJobs(): Promise<void> {
    if (!native?.lifecycle_qualified) return;
    const jobs = await sql(() => db.prepare("SELECT id,target_json FROM game_stop_jobs").all() as { id: string; target_json: string }[]);
    for (const job of jobs) {
      try {
        const target = JSON.parse(job.target_json) as GameStopTarget;
        await call(signal => native.stop(target, "owner_deleted", signal));
        await sql(() => db.prepare("DELETE FROM game_stop_jobs WHERE id=? AND target_json=?").run(job.id, job.target_json));
        owned.delete(job.id);
      } catch { /* durable minimal handoff survives account/process deletion */ }
    }
  }
  async function tick(): Promise<void> {
    if (closed || !native?.lifecycle_qualified) return;
    const rows = await sql(() => db.prepare("SELECT record_json FROM game_launches").all() as { record_json: string }[]);
    const records = rows.map(row => JSON.parse(row.record_json) as GameRecord);
    for (const record of records) if ((!["completed", "paused", "interrupted"].includes(record.state)) || !record.consumers_stopped || owned.has(record.id)) await run(record);
    await drainStopJobs();
  }
  return {
    admission, launch, resume,
    async humanSeat(actor: Actor, id: string): Promise<{ bootstrap_file: string }> {
      const game = await sql(() => read(actor, id));
      if (closed || !native?.lifecycle_qualified || !options.browser_session_qualified || game.state !== "active" || !sameIdentity(game.identity, native.identity)) throw new ServiceError("Game session is unavailable", 503);
      let bytes: string;
      try {
        bytes = await call(signal => native.humanBootstrap(structuredClone(game), signal));
        if (typeof bytes !== "string" || Buffer.byteLength(bytes) > 4096 || bytes.length === 0 || native.validateHumanBootstrap(bytes, structuredClone(game)) !== true) throw new Error("Invalid seat delivery");
      } catch { throw new ServiceError("Game session is unavailable", 503); }
      // Native adapter must validate generated SeatBootstrap and exact human
      // binding. Recheck ownership after I/O before releasing any capability.
      const current = await sql(() => read(actor, id));
      if (current.state !== "active") throw new ServiceError("Game session is unavailable", 503);
      return { bootstrap_file: bytes };
    },
    get: (actor: Actor, id: string) => sql(() => read(actor, id)),
    list: (actor: Actor) => sql(() => { authorize(actor); return (db.prepare("SELECT record_json FROM game_launches WHERE user_id=? ORDER BY rowid DESC LIMIT 100").all(actor.user_id) as { record_json: string }[]).map(row => JSON.parse(row.record_json) as GameRecord); }),
    tick: () => ticking ??= tick().finally(() => { ticking = undefined; }),
    start() { if (!timer && !closed) { timer = setInterval(() => { void this.tick().catch(() => {}); }, interval); timer.unref(); void this.tick().catch(() => {}); } },
    async close() {
      closed = true; clearInterval(timer); await ticking; await Promise.allSettled(inflight.values());
      if (native) await Promise.allSettled([...owned.values()].map(record => call(signal => native.stop(record, "shutdown", signal))));
      await drainStopJobs();
    },
  };
}
