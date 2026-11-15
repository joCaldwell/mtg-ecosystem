import type { ChildProcess } from "node:child_process";

export async function deadline<T>(work: Promise<T>, milliseconds: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try { return await Promise.race([work, new Promise<never>((_, fail) => { timer = setTimeout(() => fail(Error("deadline")), milliseconds); })]); }
  finally { clearTimeout(timer); }
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
