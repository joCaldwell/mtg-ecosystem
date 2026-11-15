import type { ChildProcess } from "node:child_process";

export async function deadline<T>(work: Promise<T>, milliseconds: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try { return await Promise.race([work, new Promise<never>((_, fail) => { timer = setTimeout(() => fail(Error("deadline")), milliseconds); })]); }
  finally { clearTimeout(timer); }
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

/** Test-driver recovery only: never returns a stale menu or submits a command.
 * A new discovery is permitted only after an observed read 429 and UI recovery. */
export async function discoverWithRateRecovery<C, T>(effects: {
  allowed: boolean; end: number; now(): number;
  healthy(): Promise<void>; capture(): C | Promise<C>;
  discover(context: C, end: number): Promise<{ phase: string; value: T }>;
  usable?(context: C, value: T): Promise<boolean>;
  rateCount(): number; recovered(end: number): Promise<number>;
}, counts: { handled: number; started: number; ready: number; freshDiscoveries: number }) {
  let recoveries = 0;
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
    if (recoveries) counts.freshDiscoveries++;
    const result = await within(() => effects.discover(context, effects.end));
    await within(effects.healthy);
    const phase = result.phase === "complete" && effects.usable && !await within(() => effects.usable!(context, result.value)) ? "stale" : result.phase;
    if (phase === "complete") return { context, value: result.value, recovered: recoveries > 0 };
    if (phase !== "stale" || !effects.allowed || effects.rateCount() <= counts.handled)
      throw Error(`discovery_${phase || "deadline"}`);
    if (recoveries === 2) throw Error("persistent_rate_recovery");
    recoveries++; counts.started++;
    // The UI callback requires a successful resume after the latest observed
    // 429, a ready connection, enabled discovery and an empty journal.
    counts.handled = await within(() => effects.recovered(effects.end));
    counts.ready++;
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
