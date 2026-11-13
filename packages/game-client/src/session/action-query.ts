import type { ActionBeginning, ActionInitiation, ActionPage, ActionQueryRequest, ActionQueryResponse, ActionQueryUnavailable, ActionScope } from '@multiverse/game-server-sdk';
import type { ReadBinding } from './read-state.ts';

/** Created by the read owner only after ownership, manifest and actual viewer
 * checks. A fresh lifetime is required on reauthorization/decision invalidation.
 * No capability, stream cursor, journal or command authority crosses this API. */
export interface ActionQueryContext {
  readonly authorization: object;
  readonly generation: object;
  readonly binding: ReadBinding;
  readonly protocolSchema: string;
  readonly token: ActionPage['token'];
}
export type ActionQueryTransport = (request: ActionQueryRequest, signal: AbortSignal) => Promise<ActionQueryResponse>;
export type ActionQueryFailure = 'unauthorized' | 'forbidden' | 'rate-limited' | 'unavailable' | 'protocol' | 'request-transport' | 'body-transport';
export interface QueryOperation {
  readonly signal: AbortSignal;
  /** Resolves at the deadline, even if a transport ignores abort. */
  readonly expired: Promise<void>;
  abort(): void;
  dispose(): void;
}
export interface ActionQueryEffects {
  current(): ActionQueryContext | null;
  /** Must return a closure over this exact authorized SDK instance. Never look up
   * replacement credentials at call time. Cleanup uses only this captured owner. */
  authorize(context: ActionQueryContext): ActionQueryTransport | null;
  operation(milliseconds: number): QueryOperation;
  wait(milliseconds: number, signal: AbortSignal): Promise<void>;
  now(): number; // monotonic milliseconds
  classify(error: unknown): ActionQueryFailure;
}
export interface ActionQueryLimits {
  readonly pageSize: number;
  readonly workPerCall: number;
  readonly calls: number;
  readonly work: number;
  readonly entries: number;
  readonly bytes: number;
  readonly lifetimeMs: number;
  readonly requestMs: number;
  readonly retries: number;
}
/** Smaller than native limits; aggregate bytes include every received envelope,
 * not just retained entries. Cleanup has one additional bounded call, no work. */
export const ACTION_QUERY_LIMITS: Readonly<ActionQueryLimits> = Object.freeze({
  pageSize: 16, workPerCall: 250, calls: 32, work: 8_000, entries: 256,
  bytes: 131_072, lifetimeMs: 20_000, requestMs: 4_000, retries: 2,
});
export type ActionQueryPhase = 'idle' | 'loading' | 'incomplete' | 'complete' | 'stale' | 'unavailable' | 'cancelled' | 'limited' | 'uncertain';
export interface ActionQuerySnapshot {
  readonly generation: number;
  readonly phase: ActionQueryPhase;
  readonly scope: Readonly<ActionScope> | null;
  readonly context: Readonly<ActionPage['context']>;
  readonly beginnings: readonly Readonly<ActionBeginning>[];
  readonly failure: ActionQueryFailure | 'binding' | 'deadline' | 'request-limit' | 'aggregate-limit' | 'budget' | null;
  readonly unavailable: Readonly<ActionQueryUnavailable> | null;
  readonly cancellation: 'none' | 'pending' | 'confirmed' | 'stale' | 'unknown';
  readonly calls: number;
  readonly work: number;
  readonly bytes: number;
}
function freeze<T>(value: T): T {
  if (value && typeof value === 'object') { for (const child of Object.values(value)) freeze(child); Object.freeze(value); }
  return value;
}
function copy<T>(value: T): T { return freeze(JSON.parse(JSON.stringify(value)) as T); }
function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value !== null && typeof value === 'object') return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonical((value as Record<string, unknown>)[key])}`).join(',')}}`;
  return JSON.stringify(value);
}
function bytes(value: unknown): number {
  let size = 0;
  for (const char of JSON.stringify(value)) { const point = char.codePointAt(0)!; size += point < 128 ? 1 : point < 2048 ? 2 : point < 65536 ? 3 : 4; }
  return size;
}
function sameScope(a: ActionScope, b: ActionScope): boolean { return a.scope === b.scope && (a.scope !== 'object' || (b.scope === 'object' && a.object === b.object)); }
function sameContext(a: ActionQueryContext | null, b: ActionQueryContext): boolean {
  return !!a && a.authorization === b.authorization && a.generation === b.generation && a.token === b.token && a.protocolSchema === b.protocolSchema
    && (['server_origin', 'protocol_version', 'match_id', 'viewer', 'engine_build_id', 'server_build_id', 'catalog_sha256'] as const).every(key => a.binding[key] === b.binding[key]);
}
function initiationKey(value: ActionInitiation): string { return canonical(value); }
function graphic(value: string): boolean { return /^[!-~]{1,128}$/.test(value); }
function validScope(scope: ActionScope): boolean { return scope.scope === 'window' || (scope.scope === 'object' && graphic(scope.object)); }
const empty = (generation = 0): ActionQuerySnapshot => freeze({ generation, phase: 'idle', scope: null, context: null, beginnings: [], failure: null, unavailable: null, cancellation: 'none', calls: 0, work: 0, bytes: 0 });
class Deadline extends Error {
  readonly dispatched: boolean;
  constructor(dispatched = false) { super(); this.dispatched = dispatched; }
}
interface Run {
  readonly id: number;
  readonly owner: ActionQueryContext;
  readonly scope: ActionScope;
  readonly query: ActionQueryTransport;
  readonly started: number;
  readonly cursors: Set<string>;
  readonly entries: Set<string>;
  operation: QueryOperation | null;
  cursor: string | null;
  inFlight: boolean;
  stopped: boolean;
  lost: boolean;
  retirement: Promise<void> | null;
  cleanup: Promise<void> | null;
  calls: number;
  work: number;
  size: number;
  retries: number;
  page: ActionPage | null;
}

/** One user-initiated bounded discovery chain. No automatic reset/restart,
 * independent match polling, persistence or response/command construction. */
export class ActionQueryController {
  readonly #effects: ActionQueryEffects;
  readonly #limits: Readonly<ActionQueryLimits>;
  readonly #listeners = new Set<() => void>();
  #snapshot = empty();
  #run: Run | null = null;
  #generation = 0;
  #intent = 0;
  constructor(effects: ActionQueryEffects, limits: Partial<ActionQueryLimits> = {}) {
    this.#effects = effects;
    const bounds = { ...ACTION_QUERY_LIMITS, ...limits };
    for (const key of Object.keys(bounds) as (keyof ActionQueryLimits)[]) {
      if (!Number.isSafeInteger(bounds[key]) || bounds[key] < (key === 'retries' ? 0 : 1) || bounds[key] > ACTION_QUERY_LIMITS[key]) throw new Error('Invalid action query client limit');
    }
    this.#limits = Object.freeze(bounds);
  }
  getSnapshot = (): ActionQuerySnapshot => this.#snapshot;
  subscribe = (listener: () => void): (() => void) => { this.#listeners.add(listener); return () => { this.#listeners.delete(listener); }; };
  #publish(run: Run, patch: Partial<ActionQuerySnapshot>): void {
    if (this.#run !== run) return;
    this.#snapshot = freeze({ ...this.#snapshot, ...patch, calls: run.calls, work: run.work, bytes: run.size });
    for (const listener of [...this.#listeners]) listener();
  }
  #current(run: Run): boolean {
    if (this.#run !== run || run.stopped) return false;
    if (!sameContext(this.#effects.current(), run.owner)) { void this.#retire(run, 'stale'); return false; }
    return true;
  }
  /** Call synchronously BEFORE publishing a read-owner/seat/decision replacement
   * or releasing its lock. Also handles null current state during initialization. */
  synchronize(): void { if (this.#run) this.#current(this.#run); }
  async stop(): Promise<void> { this.#intent++; if (this.#run) await this.#retire(this.#run, 'cancelled'); }
  async cancel(generation: number): Promise<void> { if (this.#run?.id === generation) { this.#intent++; await this.#retire(this.#run, 'cancelled'); } }
  async #retire(run: Run, phase: 'cancelled' | 'stale' | 'limited' | 'uncertain' | 'unavailable', failure: ActionQuerySnapshot['failure'] = null): Promise<void> {
    if (run.stopped) { await run.retirement; await run.cleanup; return; }
    // Abort and publication can synchronously reenter stop/start. Install the
    // barrier before either callback, including before cleanup has begun.
    let retired!: () => void;
    run.retirement = new Promise<void>(resolve => { retired = resolve; });
    run.stopped = true;
    const cursor = run.inFlight ? null : run.cursor;
    run.cursor = null;
    try {
      run.operation?.abort();
      this.#publish(run, { phase, failure, context: null, beginnings: [], cancellation: cursor ? 'pending' : run.inFlight || run.lost ? 'unknown' : 'none' });
    } finally {
      try { if (cursor) await this.#cleanup(run, cursor); else await run.cleanup; }
      finally { retired(); }
    }
  }
  #envelope(run: Run, result: ActionQueryResponse): boolean {
    return result.protocol_version === run.owner.binding.protocol_version && result.match_id === run.owner.binding.match_id && result.viewer === run.owner.binding.viewer;
  }
  #pageBinding(run: Run, page: ActionPage): boolean { return page.token === run.owner.token && sameScope(page.scope, run.scope); }
  #freshCursor(run: Run, result: ActionQueryResponse): string | null {
    return this.#envelope(run, result) && result.outcome.status === 'incomplete'
      && this.#pageBinding(run, result.outcome.page) && /^[a-f0-9]{64}$/.test(result.outcome.cursor) && !run.cursors.has(result.outcome.cursor)
      ? result.outcome.cursor : null;
  }
  async #cleanup(run: Run, cursor: string): Promise<void> {
    if (run.cleanup) return run.cleanup;
    let cleaned!: () => void;
    run.cleanup = new Promise<void>(resolve => { cleaned = resolve; });
    void (async () => {
      let operation: QueryOperation | null = null;
      try {
        const deadline = this.#effects.now() + this.#limits.requestMs;
        const active = this.#effects.operation(this.#limits.requestMs);
        operation = active;
        if (this.#effects.now() >= deadline) throw new Deadline();
        const result = await Promise.race([run.query({ operation: 'cancel', cursor }, active.signal), active.expired.then(() => { active.abort(); throw new Deadline(); })]);
        if (this.#effects.now() >= deadline) throw new Deadline();
        this.#publish(run, { cancellation: this.#envelope(run, result) && result.outcome.status === 'cancelled' ? 'confirmed'
          : this.#envelope(run, result) && result.outcome.status === 'stale' ? 'stale' : 'unknown' });
      } catch { this.#publish(run, { cancellation: 'unknown' }); }
      finally { operation?.dispose(); cleaned(); }
    })();
    return run.cleanup;
  }
  #late(run: Run, result: ActionQueryResponse): void {
    // A late successful response can reveal the ROTATED own cursor. An old
    // in-flight cursor is never replayed/cancelled speculatively. Otherwise TTL
    // (30s absolute on worker) is the only scratch-disposition guarantee.
    const cursor = this.#freshCursor(run, result);
    if ((run.stopped || run.lost) && cursor) void this.#cleanup(run, cursor);
  }
  async #send(run: Run, request: ActionQueryRequest): Promise<{ result: ActionQueryResponse; deadline: number }> {
    const now = this.#effects.now();
    const deadline = Math.min(now + this.#limits.requestMs, run.started + this.#limits.lifetimeMs);
    if (now >= deadline) throw new Deadline();
    const operation = this.#effects.operation(deadline - now);
    run.operation = operation;
    try {
      if (!this.#current(run) || this.#effects.now() >= deadline) throw new Deadline();
      run.inFlight = true;
      const pending = run.query(request, operation.signal);
      void pending.then(result => this.#late(run, result), () => {});
      const result = await Promise.race([pending, operation.expired.then(() => { operation.abort(); throw new Deadline(true); })]);
      return { result, deadline };
    }
    finally { run.inFlight = false; run.operation = null; operation.dispose(); }
  }
  async #wait(run: Run, milliseconds: number): Promise<void> {
    const remaining = this.#limits.lifetimeMs - (this.#effects.now() - run.started);
    if (remaining <= milliseconds) throw new Deadline();
    const operation = this.#effects.operation(remaining);
    run.operation = operation;
    try {
      if (!this.#current(run) || this.#effects.now() - run.started >= this.#limits.lifetimeMs) throw new Deadline();
      await Promise.race([this.#effects.wait(milliseconds, operation.signal), operation.expired.then(() => { operation.abort(); throw new Deadline(); })]);
      if (this.#effects.now() - run.started >= this.#limits.lifetimeMs) throw new Deadline();
    }
    finally { operation.dispose(); run.operation = null; }
  }
  /** Returns THIS intent's generation even if superseded while awaiting I/O.
   * Never associate a completion with whichever snapshot happens to be current. */
  async start(context: ActionQueryContext, scope: ActionScope): Promise<number | null> {
    if (!sameContext(this.#effects.current(), context)) return null;
    // Capture before any cancellation await, so an intent cannot acquire a newer
    // token/binding/scope while its predecessor is shutting down.
    const b = context.binding;
    const owner = Object.freeze({ authorization: context.authorization, generation: context.generation, protocolSchema: context.protocolSchema, token: context.token,
      binding: Object.freeze({ server_origin: b.server_origin, protocol_version: b.protocol_version, match_id: b.match_id, viewer: b.viewer,
        engine_build_id: b.engine_build_id, server_build_id: b.server_build_id, catalog_sha256: b.catalog_sha256 }) });
    const capturedScope = copy(scope);
    // Idempotent repeated start for StrictMode/duplicate intent. A stopped run
    // requires a fresh explicit start; no hidden budget replenishment.
    if (this.#run && !this.#run.stopped && sameContext(this.#run.owner, context) && sameScope(this.#run.scope, scope)) return this.#run.id;
    const intent = ++this.#intent;
    if (this.#run) await this.#retire(this.#run, 'cancelled');
    if (intent !== this.#intent || !sameContext(this.#effects.current(), owner)) return null;
    const query = this.#effects.authorize(context);
    if (!query || intent !== this.#intent || !sameContext(this.#effects.current(), owner)) return null;
    const run: Run = { id: ++this.#generation, owner, scope: capturedScope, query, started: this.#effects.now(), cursors: new Set(), entries: new Set(), operation: null,
      cursor: null, inFlight: false, stopped: false, lost: false, retirement: null, cleanup: null, calls: 0, work: 0, size: 0, retries: 0, page: null };
    this.#run = run; this.#snapshot = empty(run.id);
    this.#publish(run, { phase: 'loading', scope: run.scope });
    await this.#discover(run);
    return run.id;
  }
  async #discover(run: Run): Promise<void> {
    if (!this.#current(run)) return;
    const owner = run.owner;
    if (!graphic(owner.token) || !validScope(run.scope)) { await this.#retire(run, 'limited', 'request-limit'); return; }
    let request: ActionQueryRequest = { operation: 'start', token: owner.token, scope: run.scope, page_size: this.#limits.pageSize, work: this.#limits.workPerCall };
    try {
      while (this.#current(run)) {
        if (this.#effects.now() - run.started >= this.#limits.lifetimeMs) throw new Deadline();
        if (run.calls >= this.#limits.calls || run.work + this.#limits.workPerCall > this.#limits.work) { await this.#retire(run, 'limited', 'budget'); return; }
        if (bytes(request) > 2048) { await this.#retire(run, 'limited', 'request-limit'); return; }
        run.calls++; run.work += this.#limits.workPerCall;
        // Reserve the allowance BEFORE every attempt, including failed/retried ones.
        this.#publish(run, { phase: run.page ? 'incomplete' : 'loading' });
        if (!this.#current(run)) return;
        let result: ActionQueryResponse;
        let deadline: number;
        try { ({ result, deadline } = await this.#send(run, request)); }
        catch (error) {
          if (!this.#current(run)) return;
          if (error instanceof Deadline && !error.dispatched) { await this.#retire(run, 'limited', 'deadline'); return; }
          const failure = error instanceof Deadline ? 'deadline' : this.#effects.classify(error);
          // Only HTTP 429 is known pre-dispatch admission. 503/protocol/body/
          // request loss may have consumed/rotated the cursor: NEVER replay it.
          if (failure === 'rate-limited' && run.retries < this.#limits.retries) { run.retries++; await this.#wait(run, 1000); continue; }
          run.lost = failure !== 'rate-limited' && failure !== 'unauthorized' && failure !== 'forbidden';
          if (run.lost) run.cursor = null;
          await this.#retire(run, run.lost ? 'uncertain' : 'unavailable', failure); return;
        }
        if (!this.#current(run)) { this.#late(run, result); return; }
        // Timers can be delayed by suspension or synchronous adapter work. A
        // resolved promise is not evidence that its monotonic deadline was met.
        if (this.#effects.now() >= deadline || this.#effects.now() - run.started >= this.#limits.lifetimeMs) {
          if (result.outcome.status === 'incomplete') {
            run.cursor = this.#freshCursor(run, result); run.lost = !run.cursor;
          } else if (!this.#envelope(run, result)) { run.cursor = null; run.lost = true; }
          else if (result.outcome.status !== 'unavailable') {
            run.cursor = null;
            if (result.outcome.status === 'complete' && !this.#pageBinding(run, result.outcome.page)) run.lost = true;
          }
          await this.#retire(run, 'limited', 'deadline'); return;
        }
        run.size += bytes(result);
        if (!this.#envelope(run, result)) { run.cursor = null; run.lost = true; await this.#retire(run, 'stale', 'binding'); return; }
        const outcome = result.outcome;
        if (outcome.status === 'complete' || outcome.status === 'incomplete') {
          // Save only a correlated fresh cursor for possible bounded cleanup.
          const fresh = outcome.status === 'incomplete' && this.#pageBinding(run, outcome.page) && /^[a-f0-9]{64}$/.test(outcome.cursor) && !run.cursors.has(outcome.cursor);
          run.cursor = fresh && outcome.status === 'incomplete' ? outcome.cursor : null;
          if (outcome.status === 'incomplete' && !fresh) run.lost = true;
          const page = outcome.page;
          const keys = page.beginnings.map(entry => initiationKey(entry.initiation));
          if (!this.#pageBinding(run, page) || page.beginnings.length > this.#limits.pageSize
            || (outcome.status === 'incomplete' && (!fresh || outcome.required_minimum !== 1))
            || (page.context === null && (page.beginnings.length > 0 || outcome.status === 'complete' || run.page?.context != null))
            || (run.page?.context != null && canonical(page.context) !== canonical(run.page.context))
            || new Set(keys).size !== keys.length || keys.some(key => run.entries.has(key))) {
            if (!run.cursor) run.lost = true;
            await this.#retire(run, 'uncertain', 'protocol'); return;
          }
          if (run.size > this.#limits.bytes || run.entries.size + keys.length > this.#limits.entries) { await this.#retire(run, 'limited', 'aggregate-limit'); return; }
          for (const key of keys) run.entries.add(key);
          run.page = copy({ ...page, beginnings: [...(run.page?.beginnings ?? []), ...page.beginnings] });
          if (outcome.status === 'incomplete') run.cursors.add(outcome.cursor);
          if (this.#effects.now() >= deadline || this.#effects.now() - run.started >= this.#limits.lifetimeMs) { await this.#retire(run, 'limited', 'deadline'); return; }
          this.#publish(run, { phase: outcome.status, context: run.page.context, beginnings: run.page.beginnings, unavailable: null });
          if (!this.#current(run) || outcome.status === 'complete') return;
          request = { operation: 'continue', cursor: outcome.status === 'incomplete' ? outcome.cursor : '', work: this.#limits.workPerCall };
          await this.#wait(run, 100); // below worker's 16 charged calls/second
        } else if (outcome.status === 'unavailable') {
          if (run.size > this.#limits.bytes) { await this.#retire(run, 'limited', 'aggregate-limit'); return; }
          this.#publish(run, { unavailable: copy(outcome.reason) });
          if (!this.#current(run)) return;
          // Worker quota is checked before taking its own continuation (see
          // game-server/src/action_queries.rs). Other unavailable reasons end it.
          if (outcome.reason.kind === 'quota' && run.retries < this.#limits.retries) { run.retries++; await this.#wait(run, 1000); continue; }
          await this.#retire(run, 'unavailable'); return;
        } else {
          run.cursor = null;
          run.stopped = true;
          this.#publish(run, { phase: outcome.status, context: null, beginnings: [] }); return;
        }
      }
    } catch (error) {
      if (!this.#current(run)) return;
      await this.#retire(run, error instanceof Deadline ? 'limited' : 'uncertain', error instanceof Deadline ? 'deadline' : 'protocol');
    }
  }
}
