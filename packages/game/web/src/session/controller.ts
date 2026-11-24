import type { PublicManifest, SeatResume, StreamPage } from '@multiverse/game-server-sdk';
import type { PresentationArtifact } from '../../../generated/presentation.d.ts';
import type { ExternalStore } from './contracts.ts';
import { CommandLifecycle, disabledCommand, journalBinding, type CommandContext, type CommandEffects, type CommandSnapshot, type DecisionIntent } from './command.ts';
import { checkManifest, immutable, resumeRead, streamRead, SessionFailure, type FailureKind, type ReadBinding } from './read-state.ts';
import { MAX_READ_RETRY_MS } from './retry.ts';

export interface ReadTransport {
  /** Remaining server-requested read cooldown, confined to this authorization. */
  retryAfter?(): number;
  manifest(signal: AbortSignal): Promise<PublicManifest>;
  resume(signal: AbortSignal): Promise<SeatResume>;
  poll(after: string, signal: AbortSignal): Promise<StreamPage>;
  /** Null means an explicit missing metadata response, never failed validation. */
  presentation(signal: AbortSignal): Promise<PresentationArtifact | null>;
}
export interface Authorization { readonly binding: ReadBinding; readonly transport: ReadTransport; readonly commands?: CommandEffects }
export interface SessionEffects<Input> {
  authorize(input: Input, signal: AbortSignal): Promise<Authorization>;
  cancellation(): { readonly signal: AbortSignal; abort(): void };
  acquire(binding: ReadBinding, signal: AbortSignal): Promise<null | (() => void)>;
  wait(milliseconds: number, signal: AbortSignal): Promise<void>;
  classify(error: unknown): FailureKind;
}
export type Connection = 'idle' | 'importing' | 'owning' | 'blocked' | 'authenticating' | 'pending' | 'ready' | 'retrying' | 'ended' | FailureKind;
export interface ReadSnapshot {
  readonly command: CommandSnapshot;
  readonly connection: Connection;
  readonly binding: ReadBinding | null;
  readonly read: SeatResume | null;
  readonly metadata: PresentationArtifact | null;
  readonly metadataMissing: boolean;
  readonly retry: number;
  readonly failure: FailureKind | null;
}
const empty = (): ReadSnapshot => immutable({ command: disabledCommand(), connection: 'idle', binding: null, read: null, metadata: null, metadataMissing: false, retry: 0, failure: null });
let nextIncarnation = 0;
interface VerifiedReads { readonly manifest: PublicManifest; readonly metadata: PresentationArtifact | null }
interface Run {
  readonly cancel: ReturnType<SessionEffects<unknown>['cancellation']>;
  pollCancel: ReturnType<SessionEffects<unknown>['cancellation']> | null;
  release: (() => void) | null;
  commands: CommandLifecycle | null;
  commandsBinding?: import('./contracts.ts').JournalBinding;
  finishing: boolean;
  refresh: boolean;
  incarnation: number;
}

/** One read/cursor owner. Optional command lifecycle shares this authorization,
 * Web Lock and generation; every effect remains injected. No SDK runtime or policy. */
export class ReadSession<Input> implements ExternalStore<ReadSnapshot> {
  readonly #effects: SessionEffects<Input>;
  #snapshot = empty();
  #listeners = new Set<() => void>();
  #run: Run | null = null;
  #joinIntent = 0;
  constructor(effects: SessionEffects<Input>) { this.#effects = effects; }
  getSnapshot = (): ReadSnapshot => this.#snapshot;
  subscribe = (listener: () => void): (() => void) => { this.#listeners.add(listener); return () => this.#listeners.delete(listener); };
  #publish(snapshot: ReadSnapshot): void { this.#snapshot = immutable(snapshot); for (const listener of this.#listeners) listener(); }
  #update(patch: Partial<ReadSnapshot>): void { this.#publish({ ...this.#snapshot, ...patch }); }
  #current(run: Run): boolean { return this.#run === run && !run.cancel.signal.aborted; }
  #finish(run: Run): void {
    if (run.finishing) return;
    run.finishing = true;
    if (this.#run === run) this.#run = null;
    run.pollCancel?.abort();
    run.cancel.abort();
    run.release?.();
    run.release = null;
  };
  commandContext = (): CommandContext | null => {
    const run = this.#run, snapshot = this.#snapshot;
    if (!run || !this.#current(run) || !run.commands || snapshot.command.phase !== 'ready' || snapshot.connection !== 'ready'
      || !snapshot.binding || snapshot.read?.outcome || snapshot.read?.state.status !== 'available') return null;
    const decision = snapshot.read.state.observation.decision;
    if (!decision) return null;
    return immutable({ binding: run.commandsBinding!, incarnation: run.incarnation, revision: snapshot.read.next, token: decision.token });
  };
  submitIntent = async (intent: DecisionIntent): Promise<void> => { await this.#run?.commands?.submitIntent(intent); };
  retryCommand = async (): Promise<void> => { await this.#run?.commands?.reconcile(true); };
  cancelQueuedCommand = async (): Promise<void> => { await this.#run?.commands?.cancelQueued(); };
  stop = (): void => { this.#joinIntent++; if (this.#run) this.#finish(this.#run); this.#publish(empty()); };
  /** Every import creates a fresh authorization/generation. A stale cleanup or
   * late transport completion has no authority over a replacement session. */
  join = async (input: Input): Promise<void> => {
    this.stop();
    const intent = this.#joinIntent;
    if (intent !== this.#joinIntent) return;
    const run: Run = { cancel: this.#effects.cancellation(), pollCancel: null, release: null, commands: null, finishing: false, refresh: false, incarnation: ++nextIncarnation };
    this.#run = run;
    this.#update({ connection: 'importing' });
    const signal = run.cancel.signal;
    try {
      const { binding, transport, commands } = await this.#effects.authorize(input, signal);
      if (!this.#current(run)) return;
      this.#update({ connection: 'owning', binding });
      const release = await this.#effects.acquire(binding, signal);
      if (!this.#current(run)) { release?.(); return; }
      if (!release) { this.#update({ connection: 'blocked' }); return; }
      run.release = release;
      let initialized = false;
      // Only a command refresh in this original authorization may reuse these
      // verified immutable reads. Reconnects invalidate them before retrying.
      let verified: VerifiedReads | null = null;
      // Keep completed stages of the current verification across a hinted 429.
      // A sliding window may allow only one request after each cooldown.
      let verification: { manifest: PublicManifest; reads?: VerifiedReads } | null = null;
      let consecutiveFailures = 0;
      // A successful empty poll does not reset a collision/rate-limit budget.
      let rateLimitFailures = 0;
      let cooledDown = false;
      while (this.#current(run)) {
        if (run.refresh) initialized = false;
        const polling = initialized;
        let commandWakeup = false;
        try {
          if (!initialized) {
            this.#update({ connection: 'authenticating' });
            let assets: VerifiedReads | undefined = (run.refresh ? verified : null) ?? verification?.reads;
            if (!assets) {
              if (!verification) {
                const manifest = await transport.manifest(signal);
                if (!this.#current(run)) return;
                checkManifest(binding, manifest);
                verification = { manifest: immutable(manifest) };
              }
              const metadata = await transport.presentation(signal);
              if (!this.#current(run)) return;
              if (metadata && metadata.catalog_sha256 !== binding.catalog_sha256) throw new SessionFailure('incompatible');
              assets = verification.reads = immutable({ manifest: verification.manifest, metadata });
            }
            const refreshing = run.refresh;
            const read = resumeRead(binding, this.#snapshot.read, await transport.resume(signal));
            if (!this.#current(run)) return;
            verified = assets;
            const { metadata } = assets;
            this.#update({ read, metadata, metadataMissing: metadata === null });
            if (commands && !run.commands) {
              run.commandsBinding = journalBinding(binding, commands.protocol);
              run.commands = new CommandLifecycle({ effects: commands, binding: run.commandsBinding, signal,
                active: () => this.#current(run), context: this.commandContext,
                publish: command => this.#update({ command }), refresh: () => {
                  run.refresh = true; run.incarnation = ++nextIncarnation;
                  // A terminal command needs a new coherent resume. Do not wait
                  // for an otherwise idle 20s poll; abort is only a read wakeup,
                  // never evidence that any command was cancelled or consumed.
                  run.pollCancel?.abort();
                } });
              await run.commands.recover();
              if (!this.#current(run)) return;
            }
            if (refreshing) { run.refresh = false; run.commands?.refreshed(); }
            if (!cooledDown) verification = null;
            initialized = true;
          } else {
            const current = this.#snapshot.read!;
            const pollCancel = this.#effects.cancellation();
            run.pollCancel = pollCancel;
            let page: StreamPage;
            try { page = await transport.poll(current.next, pollCancel.signal); }
            catch (error) {
              // An HTTP/protocol failure may finish after command refresh has
              // aborted this signal. Only an explicit abort result is a wakeup;
              // preserve genuine failures for classification and revalidation.
              commandWakeup = pollCancel.signal.aborted && error instanceof Error && error.name === 'AbortError';
              throw error;
            }
            finally { if (run.pollCancel === pollCancel) run.pollCancel = null; }
            if (!this.#current(run)) return;
            this.#update({ read: streamRead(binding, current, page) });
          }
          // A successful recovery resume alone cannot renew the retry budget:
          // the next poll may still fail. Only a successful stream read does.
          if (polling) {
            verification = null;
            consecutiveFailures = 0;
            // Successful reads after an observed server cooldown establish real
            // recovery. Hintless collision loops retain their bounded budget.
            if (cooledDown) { rateLimitFailures = 0; cooledDown = false; }
          }
          const read = this.#snapshot.read!;
          this.#update({ connection: read.outcome ? 'ended' : read.state.status === 'pending' ? 'pending' : 'ready', retry: 0, failure: null });
          await run.commands?.reconcile();
          if (!this.#current(run)) return;
          if (read.outcome) return;
          // Long poll is bounded at 20s in the adapter. A minimum 100ms between
          // responses also bounds immediate empty/backlog loops and background work.
          await this.#effects.wait(100, signal);
        } catch (error) {
          if (!this.#current(run)) return;
          if (polling && run.refresh && commandWakeup) { initialized = false; continue; }
          verified = null;
          const kind = error instanceof SessionFailure ? error.kind : this.#effects.classify(error);
          if (!['rate-limited', 'unavailable', 'disconnected'].includes(kind)) throw new SessionFailure(kind);
          consecutiveFailures += 1;
          if (kind === 'rate-limited') rateLimitFailures += 1;
          const attempt = Math.max(consecutiveFailures, kind === 'rate-limited' ? rateLimitFailures : 0);
          if (attempt > 4) throw new SessionFailure(kind);
          this.#update({ connection: 'retrying', retry: attempt, failure: kind });
          const hint = kind === 'rate-limited' ? transport.retryAfter?.() ?? 0 : 0;
          const cooldown = Number.isFinite(hint) ? Math.min(MAX_READ_RETRY_MS, Math.max(0, hint)) : 0;
          if (cooldown === 0) verification = null;
          await this.#effects.wait(Math.max(500 * 2 ** (attempt - 1), cooldown), signal);
          if (!this.#current(run)) return;
          if (cooldown > 0) cooledDown = true;
          // Reconnect from one committed boundary after a read failure. Never
          // replay historical stream pages or infer command disposition here.
          initialized = false;
        }
      }
    } catch (error) {
      if (this.#current(run)) {
        const kind = error instanceof SessionFailure ? error.kind : this.#effects.classify(error);
        const clear = ['unauthorized', 'incompatible', 'invalid-file'].includes(kind);
        this.#update({ connection: kind, failure: kind, ...(clear ? { read: null, metadata: null } : {}) });
      }
    } finally { this.#finish(run); }
  };
}
