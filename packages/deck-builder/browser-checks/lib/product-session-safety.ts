import type { ChildProcess } from "node:child_process";

export async function deadline<T>(work: Promise<T>, milliseconds: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try { return await Promise.race([work, new Promise<never>((_, fail) => { timer = setTimeout(() => fail(Error("deadline")), milliseconds); })]); }
  finally { clearTimeout(timer); }
}

export function readCooldownPending(rateCount: number, resumesAtLastRate: number, successfulResumes: number) {
  return rateCount > 0 && successfulResumes <= resumesAtLastRate;
}

/** Browser-observation ledger, never a command sender or journal writer. A
 * missing execution only permits byte-identical delivery of an existing intent. */
export class SubmissionAudit<T> {
  readonly records = new Map<string, { body: string; original: T; attempts: number; rateLimited: boolean;
    missing: boolean; accepted: boolean; consumed: boolean; resolved: boolean; resumeAtRate: number }>();
  active: string | null = null;
  attempts = 0; replays = 0; rateLimits = 0; missingExecutions = 0; resolutions = 0; recovered = 0;
  attempt(id: string, body: string, original?: T) {
    let record = this.records.get(id);
    const replay = Boolean(record);
    if (record) {
      if (body !== record.body) throw Error("replay_changed_bytes");
      if (original !== undefined || this.active !== id || record.consumed || record.resolved
        || !record.rateLimited || !record.missing) throw Error("unexplained_command_replay");
      if (record.attempts >= 5) throw Error("submission_attempt_bound");
      record.missing = false; this.replays++;
    } else {
      if (this.active !== null || original === undefined) throw Error("new_intent_while_unresolved");
      record = { body, original: structuredClone(original), attempts: 0, rateLimited: false,
        missing: false, accepted: false, consumed: false, resolved: false, resumeAtRate: 0 };
      this.records.set(id, record); this.active = id;
    }
    record.attempts++; this.attempts++;
    return { replay, original: record.original };
  }
  #record(id: string) { const value = this.records.get(id); if (!value) throw Error("unrelated_command_response"); return value; }
  rateLimited(id: string, successfulResumes: number) {
    const record = this.#record(id); record.rateLimited = true; record.resumeAtRate = successfulResumes; this.rateLimits++;
  }
  missing(id: string) {
    const record = this.#record(id); this.missingExecutions++;
    if (!record.consumed && !record.resolved) record.missing = true;
  }
  accepted(id: string) { this.#record(id).accepted = true; }
  consumed(id: string) { this.#record(id).consumed = true; }
  resolve(id: string) {
    const record = this.#record(id);
    if (!record.accepted || !record.consumed || record.resolved || this.active !== id) throw Error("unproven_command_resolution");
    record.resolved = true; this.active = null; this.resolutions++;
    if (record.rateLimited) this.recovered++;
  }
  get pendingRateLimited() { return this.active !== null && this.records.get(this.active)!.rateLimited; }
}

/** Waits only for the caller's authoritative UI/receipt predicate. Observed read
 * cooldown can extend 20s to 90s; it never establishes command disposition. */
export async function waitForUiRead(effects: {
  end: number; code: string; now(): number; healthy(): void; ratePending(): boolean;
  predicate(): Promise<boolean>; wait(): Promise<void>; onCooldown(): void;
}) {
  const started = effects.now(); let extended = false;
  for (;;) {
    effects.healthy();
    if (!extended && effects.ratePending()) { extended = true; effects.onCooldown(); }
    if (effects.now() >= Math.min(effects.end, started + (extended ? 90_000 : 20_000))) throw Error(effects.code);
    if (await effects.predicate()) return extended;
    await effects.wait();
  }
}

export interface StaleRediscoveryCounts {
  native: number; local: number; started: number; ready: number; freshGenerations: number;
  contextChanges: number; tokenChanges: number; cursorChanges: number; maxClicks: number; exhausted: number;
}

/** Test-driver recovery only: never returns a stale menu or submits a command.
 * The opt-in normal-stale lane shares a three-click decision budget with rate
 * recovery. Each attempt captures its own context and discards prior results. */
export async function discoverWithRateRecovery<C, T>(effects: {
  allowed: boolean; end: number; now(): number;
  healthy(): Promise<void>; capture(): C | Promise<C>;
  discover(context: C, end: number): Promise<{ phase: string; value: T; nativeStale?: boolean }>;
  usable?(context: C, value: T): Promise<boolean>;
  /** Optional UI dispatch fence: false proves no command was sent and requires
   * discarding this menu. Never retry an attempted/unknown command here. */
  use?(context: C, value: T, end: number): Promise<boolean>;
  rateCount(): number; recovered(end: number): Promise<number>;
  normalStale?: { counts: StaleRediscoveryCounts; ready(end: number): Promise<void>;
    changed(previous: C, current: C): { token: boolean; cursor: boolean } };
}, counts: { handled: number; started: number; ready: number; freshDiscoveries: number; retired?: number }) {
  let recoveries = 0, normalRecoveries = 0, clicks = 0;
  let previous: C | undefined, nextRecovery: "rate" | "normal" | null = null;
  const within = async <V>(work: () => Promise<V>) => {
    const remaining = effects.end - effects.now();
    if (remaining <= 0) throw Error("rate_recovery_deadline");
    const value = await deadline(work(), remaining).catch(error => {
      if (error instanceof Error && error.message === "deadline") throw Error("rate_recovery_deadline");
      throw error;
    });
    if (effects.now() >= effects.end) throw Error("rate_recovery_deadline");
    return value;
  };
  for (;;) {
    await within(effects.healthy);
    const context = await within(async () => effects.capture());
    if (nextRecovery === "rate") counts.freshDiscoveries++;
    const normal = effects.normalStale;
    if (normal && clicks) {
      normal.counts.freshGenerations++;
      const change = normal.changed(previous!, context);
      if (change.token || change.cursor) normal.counts.contextChanges++;
      if (change.token) normal.counts.tokenChanges++;
      if (change.cursor) normal.counts.cursorChanges++;
    }
    previous = context; clicks++;
    if (normal) normal.counts.maxClicks = Math.max(normal.counts.maxClicks, clicks);
    const result = await within(() => effects.discover(context, effects.end));
    await within(effects.healthy);
    const rateAtMenuCheck = effects.rateCount();
    let phase = result.phase === "complete" && effects.usable && !await within(() => effects.usable!(context, result.value)) ? "stale" : result.phase;
    let nativeStale = result.nativeStale;
    if (phase === "complete") {
      // A coherent usable menu closes the prior rate episode, even if it did
      // not require rediscovery. Never spend old unused 429 evidence on an
      // unrelated stale menu later. Retirement is not a recovery claim.
      // A new429 arriving during/after the menu check belongs to the later UI
      // dispatch attempt, not this completed menu's retired rate episode.
      const retired = Math.max(0, rateAtMenuCheck - counts.handled);
      if (retired) { counts.handled += retired; counts.retired = (counts.retired ?? 0) + retired; }
      if (!effects.use || await within(() => effects.use!(context, result.value, effects.end)))
        return { context, value: result.value, recovered: recoveries > 0,
          ...(normal ? { rediscovered: normalRecoveries > 0 } : {}) };
      phase = "stale"; nativeStale = false;
    }
    if (normal && phase === "stale") normal.counts[nativeStale ? "native" : "local"]++;
    if (phase !== "stale" || !effects.allowed)
      throw Error(`discovery_${phase || "deadline"}`);
    if (normal && clicks === 3) { normal.counts.exhausted++; throw Error("persistent_discovery_stale"); }
    if (effects.rateCount() <= counts.handled) {
      if (!normal) throw Error("discovery_stale");
      normalRecoveries++; normal.counts.started++;
      await within(() => normal.ready(effects.end));
      normal.counts.ready++; nextRecovery = "normal";
      continue;
    }
    if (recoveries === 2) throw Error("persistent_rate_recovery");
    recoveries++; counts.started++;
    // The UI callback requires a successful resume after the latest observed
    // 429, a ready connection, enabled discovery and an empty journal.
    counts.handled = await within(() => effects.recovered(effects.end));
    counts.ready++; nextRecovery = "rate";
  }
}

/** Receipt-only parsing; this never schedules or changes product I/O. */
export function boundedRetryAfter(value: string | null, now: number): number | null {
  if (value === null) return null;
  const seconds = /^\d+$/.test(value) ? Number(value) : Math.ceil((Date.parse(value) - now) / 1_000);
  return Number.isSafeInteger(seconds) && seconds >= 0 && seconds <= 60 ? seconds : null;
}
class ShutdownCancellation extends Error {}
/** Cancellation is classified only at the response-body transport boundary.
 * Syntax/schema/context errors and page errors never become shutdown noise. */
export class ObserverAudit {
  closing = false;
  protocolFailed = false;
  pageErrors = 0;
  expectedCancellations = 0;
  readonly #jobs = new Set<Promise<void>>();
  expectedAbort(errorText?: string) { return this.closing && ["net::ERR_ABORTED", "NS_BINDING_ABORTED"].includes(errorText ?? ""); }
  track(operation: () => Promise<void>) {
    const job = Promise.resolve().then(operation).catch(error => {
      if (this.closing && error instanceof ShutdownCancellation) this.expectedCancellations++;
      else this.protocolFailed = true;
    });
    this.#jobs.add(job); void job.finally(() => this.#jobs.delete(job));
    return job;
  }
  async json(response: { json(): Promise<any>; request(): { failure(): { errorText: string } | null } }) {
    try { return await response.json(); }
    catch (error) {
      if (this.expectedAbort(response.request().failure()?.errorText) && !(error instanceof SyntaxError)) throw new ShutdownCancellation();
      throw error;
    }
  }
  async drain(milliseconds: number) {
    const end = Date.now() + milliseconds;
    do {
      if (this.#jobs.size) await deadline(Promise.all([...this.#jobs]), Math.max(1, end - Date.now()));
      // Context/browser close is awaited first. Flush queued response/pageerror
      // callbacks as well as jobs created while an earlier body was draining.
      await new Promise<void>(done => setImmediate(done));
    } while (this.#jobs.size && Date.now() < end);
    if (this.#jobs.size) throw Error("observer_drain_deadline");
  }
  verdict(failure: string, complete: boolean) {
    const reason = failure || (this.pageErrors ? "page_error" : this.protocolFailed ? "protocol_failed" : !complete ? "all_command_proofs" : "");
    return { passed: !reason, failure: reason };
  }
}

/** Always escalates a stalled owned child and releases IPC, then independently
 * bounds native/lease verification. Escalation remains an incomplete cleanup. */
export async function finishOwnedChild(child: ChildProcess, join: (signal: AbortSignal) => Promise<number>,
  limits = { graceful: 40_000, term: 20_000, kill: 5_000, join: 25_000 }) {
  let exited = child.exitCode !== null || child.signalCode !== null;
  const closed = exited ? Promise.resolve() : new Promise<void>(done => child.once("close", () => { exited = true; done(); }));
  let escalated = false, leases = 0, verified = false;
  try {
    if (child.connected) { try { child.send({ stop: true }, () => {}); } catch { /* escalation still runs */ } }
    try { await deadline(closed, limits.graceful); }
    catch {
      escalated = true; child.kill("SIGTERM");
      try { await deadline(closed, limits.term); }
      catch { child.kill("SIGKILL"); await deadline(closed, limits.kill).catch(() => {}); }
    }
  } finally {
    if (child.connected) { try { child.disconnect(); } catch { /* channel already closing */ } }
    if (!exited) child.unref();
    const abort = new AbortController();
    try { leases = await deadline(Promise.resolve().then(() => join(abort.signal)), limits.join); verified = true; }
    catch { verified = false; }
    finally { abort.abort(); }
  }
  return { complete: exited && !escalated && child.exitCode === 0 && verified, exited, escalated,
    ipcDisconnected: !child.connected, verified, leases };
}
