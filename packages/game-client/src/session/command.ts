import type { CommandEnvelope, CommandExecution, PlayerResponse, RejectionReason, SubmissionResult } from '@multiverse/game-server-sdk';
import type { CommandJournal, DefinitiveDisposition, JournalBinding, JournalRecord } from './contracts.ts';
import { JournalFailure, sameBinding, type CommandCodec, type JournalFailureKind } from './journal.ts';
import { immutable } from './read-state.ts';

export interface CommandContext {
  readonly binding: JournalBinding;
  readonly incarnation: number;
  readonly revision: string;
  readonly token: string;
}
/** Only a new, explicit human callback creates an envelope. No stored draft or
 * automatic policy may invoke this action after a rejection or new decision. */
export interface DecisionIntent {
  readonly kind: 'decision';
  readonly source: 'human';
  readonly context: CommandContext;
  readonly response: PlayerResponse;
}
export interface CommandTransport {
  submitExact(exactBody: string, signal: AbortSignal): Promise<SubmissionResult>;
  execution(id: string, signal: AbortSignal): Promise<CommandExecution | null>;
  cancelQueued(id: string, signal: AbortSignal): Promise<SubmissionResult>;
}
export interface CommandEffects {
  readonly journal: CommandJournal<CommandEnvelope>;
  readonly codec: CommandCodec;
  readonly transport: CommandTransport;
  readonly protocol: string;
  readonly protocolVersion: number;
  newId(): string;
}
export type CommandPhase = 'disabled' | 'scanning' | 'ready' | 'persisting' | 'unresolved' | 'sending' | 'awaiting-refresh' | 'blocked';
export interface CommandSnapshot {
  readonly phase: CommandPhase;
  readonly commandId: string | null;
  readonly disposition: DefinitiveDisposition | null;
  readonly rejection: RejectionReason | null;
  readonly failure: JournalFailureKind | 'binding' | 'transport' | null;
}
export const disabledCommand = (): CommandSnapshot => ({ phase: 'disabled', commandId: null, disposition: null, rejection: null, failure: null });
export function journalBinding(binding: import('./read-state.ts').ReadBinding, protocol: string): JournalBinding {
  return immutable({ origin: binding.server_origin, match: binding.match_id, seat: String(binding.viewer), engineBuild: binding.engine_build_id,
    serverBuild: binding.server_build_id, catalog: binding.catalog_sha256, protocol });
}
function sameContext(a: CommandContext | null, b: CommandContext): boolean {
  return !!a && a.incarnation === b.incarnation && a.revision === b.revision && a.token === b.token && sameBinding(a.binding, b.binding);
}
/** Owned by the existing read generation. No independent polling, cursor, timer,
 * network, database, SDK runtime or browser dependencies. */
export class CommandLifecycle {
  readonly #effects: CommandEffects;
  readonly #binding: JournalBinding;
  readonly #signal: AbortSignal;
  readonly #active: () => boolean;
  readonly #context: () => CommandContext | null;
  readonly #publish: (snapshot: CommandSnapshot) => void;
  readonly #refresh: () => void;
  #state: CommandSnapshot = { ...disabledCommand(), phase: 'scanning' };
  #record: JournalRecord<CommandEnvelope> | null = null;
  #busy = false;
  #attempts = 0;
  constructor(options: { effects: CommandEffects; binding: JournalBinding; signal: AbortSignal; active(): boolean;
    context(): CommandContext | null; publish(snapshot: CommandSnapshot): void; refresh(): void }) {
    this.#effects = options.effects; this.#binding = options.binding; this.#signal = options.signal;
    this.#active = options.active; this.#context = options.context; this.#publish = options.publish; this.#refresh = options.refresh;
  }
  #update(patch: Partial<CommandSnapshot>): void {
    if (!this.#active()) return;
    this.#state = immutable({ ...this.#state, ...patch }); this.#publish(this.#state);
  }
  #fail(error: unknown): void {
    this.#update({ phase: error instanceof JournalFailure || !this.#record ? 'blocked' : 'unresolved',
      failure: error instanceof JournalFailure ? error.kind : 'transport' });
  }
  async recover(): Promise<void> {
    this.#update({ phase: 'scanning' });
    try {
      if (!this.#active()) return;
      // Seat-only scan happens BEFORE comparing/selecting current build bindings.
      const records = await this.#effects.journal.unresolved(this.#binding);
      if (!this.#active()) return;
      if (records.length > 1) throw new JournalFailure('corrupt');
      this.#record = records[0] ?? null;
      if (this.#record && !sameBinding(this.#record.binding, this.#binding)) {
        this.#update({ phase: 'blocked', commandId: this.#record.envelope.command_id, failure: 'binding' }); return;
      }
      this.#update({ phase: this.#record ? 'unresolved' : 'ready', commandId: this.#record?.envelope.command_id ?? null });
    } catch (error) { this.#fail(error); }
  }
  /** Only a coherent resume obtained AFTER terminal resolution unlocks a fresh
   * explicit intent. A changed observation alone never resolves an uncertainty. */
  refreshed(): void {
    if (this.#state.phase === 'awaiting-refresh') this.#update({ phase: 'ready', commandId: null, failure: null });
  }
  async #resolve(disposition: DefinitiveDisposition): Promise<void> {
    if (!this.#active()) return;
    const record = this.#record!;
    await this.#effects.journal.resolve(record, disposition);
    if (!this.#active()) return;
    this.#record = null;
    this.#update({ phase: 'awaiting-refresh', disposition, failure: null });
    if (this.#active()) this.#refresh();
  }
  async #submission(result: SubmissionResult): Promise<void> {
    if (!this.#active()) return;
    if (result.command_id !== this.#record?.envelope.command_id) throw new Error('command identity');
    if (result.status === 'accepted') this.#update({ phase: 'unresolved', failure: null });
    else { this.#update({ rejection: result.reason }); await this.#resolve(result.reason === 'cancelled' ? 'cancelled' : 'rejected'); }
  }
  async submitIntent(intent: DecisionIntent): Promise<void> {
    if (!this.#active() || this.#busy || this.#state.phase !== 'ready' || intent.kind !== 'decision' || intent.source !== 'human'
      || !sameContext(this.#context(), intent.context)) return;
    this.#busy = true;
    this.#update({ phase: 'persisting', disposition: null, rejection: null, failure: null });
    try {
      if (!this.#active()) return;
      const prepared = this.#effects.codec.prepare({ protocol_version: this.#effects.protocolVersion, command_id: this.#effects.newId(), expected_revision: intent.context.revision,
        command: { kind: 'decision', token: intent.context.token, response: intent.response } });
      const record: JournalRecord<CommandEnvelope> = immutable({ version: 1, binding: this.#binding, ...prepared });
      if (!this.#active()) return;
      await this.#effects.journal.persist(record);
      // Even if stop/seat-switch happened during commit, the durable slot stays.
      if (!this.#active()) return;
      this.#record = record; this.#attempts = 0;
      this.#update({ phase: 'sending', commandId: record.envelope.command_id });
      if (!this.#active()) return;
      await this.#submission(await this.#effects.transport.submitExact(record.exactBody, this.#signal));
    } catch (error) { this.#fail(error); }
    finally { this.#busy = false; }
  }
  /** Invoked by the single read owner after a committed read, or explicitly by
   * the human retry callback. Absent execution row is NEVER a resolution. */
  async reconcile(manual = false): Promise<void> {
    if (!this.#active() || this.#busy || !this.#record || this.#state.failure === 'binding'
      || (this.#state.phase === 'blocked' && !manual) || (!manual && this.#attempts >= 4)) return;
    this.#busy = true; this.#attempts++;
    try {
      const record = this.#record;
      const execution = await this.#effects.transport.execution(record.envelope.command_id, this.#signal);
      if (!this.#active()) return;
      if (execution && execution.command_id !== record.envelope.command_id) throw new Error('command identity');
      if (!execution) {
        this.#update({ phase: 'sending' });
        if (!this.#active()) return;
        await this.#submission(await this.#effects.transport.submitExact(record.exactBody, this.#signal));
      } else {
        // Authority: game-server/src/protocol.rs CommandExecutionStatus and
        // worker.rs durable execution updates. No observation/token heuristics.
        switch (execution.status.status) {
          case 'pending': this.#update({ phase: 'unresolved', failure: null }); break;
          case 'consumed_by_rules': await this.#resolve('consumed'); break;
          case 'discarded_by_departure': await this.#resolve('departure-discarded'); break;
          case 'cancelled': await this.#resolve('cancelled'); break;
          case 'rejected': this.#update({ rejection: execution.status.reason }); await this.#resolve(execution.status.reason === 'cancelled' ? 'cancelled' : 'rejected'); break;
        }
      }
    } catch (error) { this.#fail(error); }
    finally { this.#busy = false; }
  }
  /** Requests cancellation of a SERVER-QUEUED command. It never cancels a local
   * draft, clears a missing command, or reverses accepted rules input. */
  async cancelQueued(): Promise<void> {
    if (!this.#active() || this.#busy || !this.#record || this.#state.failure === 'binding') return;
    this.#busy = true;
    try { await this.#submission(await this.#effects.transport.cancelQueued(this.#record.envelope.command_id, this.#signal)); }
    catch (error) { this.#fail(error); }
    finally { this.#busy = false; }
  }
}
