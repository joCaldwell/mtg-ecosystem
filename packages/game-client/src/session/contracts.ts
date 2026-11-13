/** Client-owned interfaces only. Instantiate Envelope/Cursor/View from generated
 * SDK types after the browser contract is pinned; these are not wire DTOs. */
export interface JournalBinding {
  readonly origin: string;
  readonly match: string;
  readonly seat: string;
  readonly engineBuild: string;
  readonly serverBuild: string;
  readonly catalog: string;
  readonly protocol: string;
}

export type SeatBinding = Pick<JournalBinding, 'origin' | 'match' | 'seat'>;
export interface JournalRecord<Envelope> {
  readonly version: 1;
  readonly binding: JournalBinding;
  /** Exact SDK-validated serialized bytes; never rebuild a retry with new revision. */
  readonly exactBody: string;
  readonly envelope: Envelope;
}
export type DefinitiveDisposition = 'consumed' | 'rejected' | 'cancelled' | 'departure-discarded';

/** IndexedDB atomically enforces one unresolved record
 * by SeatBinding across builds. persist rejects on conflict/failure before send.
 * An accepted acknowledgment, 404, local stop or elapsed time never calls resolve.
 * No capability, observation or diagnostics belongs in a record. */
export interface CommandJournal<Envelope> {
  unresolved(binding: SeatBinding): Promise<readonly JournalRecord<Envelope>[]>;
  persist(record: JournalRecord<Envelope>): Promise<void>;
  resolve(record: JournalRecord<Envelope>, disposition: DefinitiveDisposition): Promise<void>;
}

/** Immutable stable snapshots for eventual useSyncExternalStore binding. */
export interface ExternalStore<Snapshot> {
  getSnapshot(): Snapshot;
  subscribe(listener: () => void): () => void;
}
