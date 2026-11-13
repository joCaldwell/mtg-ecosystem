import type { PublicManifest, SeatResume, StreamPage } from '@multiverse/game-server-sdk';
import type { PresentationArtifact } from '../../generated/presentation.d.ts';
import type { ExternalStore } from './contracts.ts';
import { CommandLifecycle, disabledCommand, journalBinding, type CommandContext, type CommandEffects, type CommandSnapshot, type DecisionIntent } from './command.ts';
import { checkManifest, immutable, resumeRead, streamRead, SessionFailure, type FailureKind, type ReadBinding } from './read-state.ts';

export interface ReadTransport {
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
interface Run {
  readonly cancel: ReturnType<SessionEffects<unknown>['cancellation']>;
  release: (() => void) | null;
  commands: CommandLifecycle | null;
  commandsBinding?: import('./contracts.ts').JournalBinding;
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
  constructor(effects: SessionEffects<Input>) { this.#effects = effects; }
  getSnapshot = (): ReadSnapshot => this.#snapshot;
  subscribe = (listener: () => void): (() => void) => { this.#listeners.add(listener); return () => this.#listeners.delete(listener); };
  #publish(snapshot: ReadSnapshot): void { this.#snapshot = immutable(snapshot); for (const listener of this.#listeners) listener(); }
  #update(patch: Partial<ReadSnapshot>): void { this.#publish({ ...this.#snapshot, ...patch }); }
  #current(run: Run): boolean { return this.#run === run && !run.cancel.signal.aborted; }
  #finish(run: Run): void {
    run.cancel.abort(); run.release?.(); run.release = null;
    if (this.#run === run) this.#run = null;
  }
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
  stop = (): void => { if (this.#run) this.#finish(this.#run); this.#publish(empty()); };
  /** Every import creates a fresh authorization/generation. A stale cleanup or
   * late transport completion has no authority over a replacement session. */
  join = async (input: Input): Promise<void> => {
    this.stop();
    const run: Run = { cancel: this.#effects.cancellation(), release: null, commands: null, refresh: false, incarnation: ++nextIncarnation };
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
      let consecutiveFailures = 0;
      // A successful empty poll does not reset a collision/rate-limit budget.
      let rateLimitFailures = 0;
      while (this.#current(run)) {
        if (run.refresh) initialized = false;
        const polling = initialized;
        try {
          if (!initialized) {
            this.#update({ connection: 'authenticating' });
            const manifest = await transport.manifest(signal);
            if (!this.#current(run)) return;
            checkManifest(binding, manifest);
            const metadata = await transport.presentation(signal);
            if (!this.#current(run)) return;
            if (metadata && metadata.catalog_sha256 !== binding.catalog_sha256) throw new SessionFailure('incompatible');
            const refreshing = run.refresh;
            const read = resumeRead(binding, this.#snapshot.read, await transport.resume(signal));
            if (!this.#current(run)) return;
            this.#update({ read, metadata, metadataMissing: metadata === null });
            if (commands && !run.commands) {
              run.commandsBinding = journalBinding(binding, commands.protocol);
              run.commands = new CommandLifecycle({ effects: commands, binding: run.commandsBinding, signal,
                active: () => this.#current(run), context: this.commandContext,
                publish: command => this.#update({ command }), refresh: () => { run.refresh = true; run.incarnation = ++nextIncarnation; } });
              await run.commands.recover();
              if (!this.#current(run)) return;
            }
            if (refreshing) { run.refresh = false; run.commands?.refreshed(); }
            initialized = true;
          } else {
            const current = this.#snapshot.read!;
            const page = await transport.poll(current.next, signal);
            if (!this.#current(run)) return;
            this.#update({ read: streamRead(binding, current, page) });
          }
          // A successful recovery resume alone cannot renew the retry budget:
          // the next poll may still fail. Only a successful stream read does.
          if (polling) consecutiveFailures = 0;
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
          const kind = error instanceof SessionFailure ? error.kind : this.#effects.classify(error);
          if (!['rate-limited', 'unavailable', 'disconnected'].includes(kind)) throw new SessionFailure(kind);
          consecutiveFailures += 1;
          if (kind === 'rate-limited') rateLimitFailures += 1;
          const attempt = Math.max(consecutiveFailures, kind === 'rate-limited' ? rateLimitFailures : 0);
          if (attempt > 4) throw new SessionFailure(kind);
          this.#update({ connection: 'retrying', retry: attempt, failure: kind });
          await this.#effects.wait(500 * 2 ** (attempt - 1), signal);
          if (!this.#current(run)) return;
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
