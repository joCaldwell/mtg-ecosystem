import type { ActionScope, PublicManifest, SeatResume, StreamPage } from '@multiverse/game-server-sdk';
import type { PresentationArtifact } from '../../generated/presentation.d.ts';
import { ActionQueryController, type ActionQueryContext, type ActionQueryEffects, type ActionQuerySnapshot, type ActionQueryTransport } from './action-query.ts';
import { actionResponse, type LiveActionDraft } from './live-action.ts';
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
export interface ActionAuthorization extends Pick<ActionQueryEffects, 'operation' | 'now' | 'classify'> { readonly protocolSchema: string; readonly query: ActionQueryTransport }
export interface Authorization { readonly actions?: ActionAuthorization; readonly binding: ReadBinding; readonly transport: ReadTransport; readonly commands?: CommandEffects }
export interface SessionEffects<Input> {
  authorize(input: Input, signal: AbortSignal): Promise<Authorization>;
  cancellation(): { readonly signal: AbortSignal; abort(): void };
  acquire(binding: ReadBinding, signal: AbortSignal): Promise<null | (() => void)>;
  wait(milliseconds: number, signal: AbortSignal): Promise<void>;
  classify(error: unknown): FailureKind;
}
export type Connection = 'idle' | 'importing' | 'owning' | 'blocked' | 'authenticating' | 'pending' | 'ready' | 'retrying' | 'ended' | FailureKind;
export interface ReadSnapshot {
  readonly actions: ActionQuerySnapshot | null;
  readonly command: CommandSnapshot;
  readonly connection: Connection;
  readonly binding: ReadBinding | null;
  readonly read: SeatResume | null;
  readonly metadata: PresentationArtifact | null;
  readonly metadataMissing: boolean;
  readonly retry: number;
  readonly failure: FailureKind | null;
}
const empty = (): ReadSnapshot => immutable({ actions: null, command: disabledCommand(), connection: 'idle', binding: null, read: null, metadata: null, metadataMissing: false, retry: 0, failure: null });
let nextIncarnation = 0;
interface VerifiedReads { readonly manifest: PublicManifest; readonly metadata: PresentationArtifact | null }
interface Run {
  readonly cancel: ReturnType<SessionEffects<unknown>['cancellation']>;
  pollCancel: ReturnType<SessionEffects<unknown>['cancellation']> | null;
  release: (() => void) | null;
  commands: CommandLifecycle | null;
  commandsBinding?: import('./contracts.ts').JournalBinding;
  queries: Set<Promise<unknown>>;
  actionAuthorization?: ActionAuthorization;
  authorization: object;
  actions: ActionQueryController | null;
  queryContext: ActionQueryContext | null;
  queryCommand: CommandContext | null;
  draft: LiveActionDraft | null;
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
  #cleanup: Promise<void> = Promise.resolve();
  #joinIntent = 0;
  constructor(effects: SessionEffects<Input>) { this.#effects = effects; }
  getSnapshot = (): ReadSnapshot => this.#snapshot;
  subscribe = (listener: () => void): (() => void) => { this.#listeners.add(listener); return () => this.#listeners.delete(listener); };
  #publish(snapshot: ReadSnapshot): void { this.#snapshot = immutable(snapshot); if (this.#run) this.#synchronizeActions(this.#run); for (const listener of this.#listeners) listener(); }
  #update(patch: Partial<ReadSnapshot>): void { this.#publish({ ...this.#snapshot, ...patch }); }
  #current(run: Run): boolean { return this.#run === run && !run.cancel.signal.aborted; }
  #synchronizeActions(run: Run): void {
    const command = this.commandContext();
    if (JSON.stringify(command) !== JSON.stringify(run.queryCommand)) {
      run.queryCommand = command;
      run.queryContext = command && run.actionAuthorization && this.#snapshot.binding ? Object.freeze({ authorization: run.authorization, generation: Object.freeze({}), binding: this.#snapshot.binding, protocolSchema: run.actionAuthorization.protocolSchema, token: command.token }) : null;
      run.draft = null;
    }
    run.actions?.synchronize();
  }
  #finish(run: Run): void {
    if (run.finishing) return;
    run.finishing = true;
    if (this.#run === run) this.#run = null;
    run.pollCancel?.abort();
    run.queryContext = null; run.draft = null;
    const release = () => { run.cancel.abort(); run.release?.(); run.release = null; };
    // Keep the exclusive lock until captured-SDK cursor cleanup settles. No new
    // authorization can race a prior owner's cancellation request.
    if (run.actions) {
      run.actions.synchronize();
      this.#cleanup = (async () => {
        // An older retired generation can reveal its cursor while these
        // requests settle, registering another cleanup request. Drain the live
        // set rather than one snapshot before releasing exclusive ownership.
        for (;;) {
          await run.actions!.stop();
          const pending = [...run.queries];
          if (!pending.length) break;
          await Promise.allSettled(pending);
        }
      })().finally(release);
    } else release();
  }
  startActions = async (scope: ActionScope = { scope: 'window' }): Promise<LiveActionDraft | null> => {
    const run = this.#run, context = this.commandContext(), query = run?.queryContext;
    if (!run?.actions || !context || !query || !this.#current(run)) return null;
    const captured = immutable({ command: context, query, scope: JSON.parse(JSON.stringify(scope)) as ActionScope });
    const generation = await run.actions.start(query, captured.scope);
    if (!this.#current(run) || run.queryContext !== query || generation === null || run.actions.getSnapshot().generation !== generation) return null;
    const draft: LiveActionDraft = Object.freeze({ ...captured, generation });
    run.draft = draft;
    return draft;
  };
  cancelActions = async (generation: number): Promise<void> => {
    const run = this.#run;
    if (!run?.actions || run.actions.getSnapshot().generation !== generation) return;
    run.draft = null;
    await run.actions.cancel(generation);
  };
  submitAction = async (draft: LiveActionDraft, index: number): Promise<void> => {
    const run = this.#run, actions = run?.actions?.getSnapshot();
    if (!run || run.draft !== draft || run.queryContext !== draft.query || !actions || actions.generation !== draft.generation
      || actions.phase !== 'complete' || JSON.stringify(actions.scope) !== JSON.stringify(draft.scope)) return;
    const beginning = actions.beginnings[index];
    const response = beginning && actionResponse(beginning, actions.context, draft.scope);
    if (!response) return;
    // The normal gate revalidates ORIGINAL command context, persists exact SDK
    // bytes and only then submits. No callback may obtain a replacement context.
    await this.submitIntent({ kind: 'decision', source: 'human', context: draft.command, response });
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
    await this.#cleanup;
    if (intent !== this.#joinIntent) return;
    const run: Run = { cancel: this.#effects.cancellation(), pollCancel: null, release: null, commands: null, queries: new Set(), actions: null, authorization: Object.freeze({}), queryContext: null, queryCommand: null, draft: null, finishing: false, refresh: false, incarnation: ++nextIncarnation };
    this.#run = run;
    this.#update({ connection: 'importing' });
    const signal = run.cancel.signal;
    try {
      const { binding, transport, commands, actions } = await this.#effects.authorize(input, signal);
      if (!this.#current(run)) return;
      this.#update({ connection: 'owning', binding });
      const release = await this.#effects.acquire(binding, signal);
      if (!this.#current(run)) { release?.(); return; }
      if (!release) { this.#update({ connection: 'blocked' }); return; }
      run.release = release;
      if (actions && commands && actions.protocolSchema === commands.protocol) {
        run.actionAuthorization = actions;
        run.actions = new ActionQueryController({ ...actions, wait: this.#effects.wait,
          current: () => this.#current(run) ? run.queryContext : null,
          authorize: context => context.authorization === run.authorization ? (request, querySignal) => {
            if (!run.release) return Promise.reject(new SessionFailure('disconnected'));
            const operation = actions.operation(4_000), cancel = this.#effects.cancellation();
            const abort = () => cancel.abort();
            for (const signal of [querySignal, operation.signal]) {
              if (signal.aborted) abort();
              signal.addEventListener('abort', abort, { once: true });
            }
            // Register before calling injected transport, which can synchronously
            // stop/replace its owner. Cleanup alone may use a retired owner.
            const response = Promise.resolve().then(() => {
              if (!run.release || cancel.signal.aborted || (request.operation !== 'cancel'
                && (!this.#current(run) || run.queryContext?.generation !== context.generation))) throw new SessionFailure('disconnected');
              return actions.query(request, cancel.signal);
            });
            const pending = Promise.race([response, operation.expired.then(() => { cancel.abort(); throw new SessionFailure('disconnected'); })]);
            run.queries.add(pending);
            void pending.finally(() => {
              operation.dispose();
              querySignal.removeEventListener('abort', abort); operation.signal.removeEventListener('abort', abort);
              run.queries.delete(pending);
            }).catch(() => {});
            return pending;
          } : null });
        run.actions.subscribe(() => { if (this.#current(run)) this.#update({ actions: run.actions!.getSnapshot() }); });
      }
      let initialized = false;
      // Only a command refresh in this original authorization may reuse these
      // verified immutable reads. Reconnects invalidate them before retrying.
      let verified: VerifiedReads | null = null;
      let consecutiveFailures = 0;
      // A successful empty poll does not reset a collision/rate-limit budget.
      let rateLimitFailures = 0;
      while (this.#current(run)) {
        if (run.refresh) initialized = false;
        const polling = initialized;
        try {
          if (!initialized) {
            this.#update({ connection: 'authenticating' });
            let assets: VerifiedReads | null = run.refresh ? verified : null;
            if (!assets) {
              const manifest = await transport.manifest(signal);
              if (!this.#current(run)) return;
              checkManifest(binding, manifest);
              const metadata = await transport.presentation(signal);
              if (!this.#current(run)) return;
              if (metadata && metadata.catalog_sha256 !== binding.catalog_sha256) throw new SessionFailure('incompatible');
              assets = immutable({ manifest, metadata });
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
            initialized = true;
          } else {
            const current = this.#snapshot.read!;
            const pollCancel = this.#effects.cancellation();
            run.pollCancel = pollCancel;
            let page: StreamPage;
            try { page = await transport.poll(current.next, pollCancel.signal); }
            finally { if (run.pollCancel === pollCancel) run.pollCancel = null; }
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
          if (polling && run.refresh) { initialized = false; continue; }
          verified = null;
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
