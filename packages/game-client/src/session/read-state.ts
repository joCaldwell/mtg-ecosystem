import type { Observation, PublicManifest, SeatBootstrap, SeatResume, StreamPage } from '@multiverse/game-server-sdk';

/** Capability-free authorization identity. Never pass the imported file to views. */
export type ReadBinding = Readonly<Omit<SeatBootstrap, 'capability' | 'bootstrap_version'>>;
export type FailureKind = 'invalid-file' | 'incompatible' | 'unauthorized' | 'rate-limited' | 'unavailable' | 'disconnected' | 'locks-unavailable';
export class SessionFailure extends Error {
  readonly kind: FailureKind;
  constructor(kind: FailureKind) { super(kind); this.kind = kind; }
}

/** These are intentionally tested read/display capabilities, not gameplay coverage. */
export const READ_PROFILES = ['commander-foundations-v1', 'commander-attachments-v1'] as const;
export const OBSERVATION_VERSION = 16;

export function immutable<T>(value: T): T {
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) immutable(child);
    Object.freeze(value);
  }
  return value;
}

function sameIdentity(binding: ReadBinding, value: PublicManifest | SeatResume): void {
  for (const key of ['protocol_version', 'match_id', 'engine_build_id', 'server_build_id', 'catalog_sha256'] as const) {
    if (value[key] !== binding[key]) throw new SessionFailure('incompatible');
  }
}
export function checkManifest(binding: ReadBinding, manifest: PublicManifest): void {
  sameIdentity(binding, manifest);
  if (!READ_PROFILES.some(profile => profile === manifest.profile)) throw new SessionFailure('incompatible');
}
function checkObservation(binding: ReadBinding, observation: Observation): void {
  if (observation.observation_version !== OBSERVATION_VERSION || observation.view.viewer !== binding.viewer
    || (observation.decision !== null && observation.decision.player !== binding.viewer)) throw new SessionFailure('incompatible');
}

/** A resume is installed atomically. Pending/lifecycle-only reads cannot erase a
 * previously committed view in the same authorization. No command disposition is inferred. */
export function resumeRead(binding: ReadBinding, previous: SeatResume | null, incoming: SeatResume): SeatResume {
  sameIdentity(binding, incoming);
  if (incoming.viewer !== binding.viewer || (previous && BigInt(incoming.next) < BigInt(previous.next))) throw new SessionFailure('incompatible');
  if (incoming.state.status === 'available') checkObservation(binding, incoming.state.observation);
  return immutable({ ...incoming, state: incoming.state.status === 'pending' && previous ? previous.state : incoming.state });
}

/** SDK owns wire/cursor validation; session also binds every replacement to its
 * authorization. Validate the entire page before publishing any of it. */
export function streamRead(binding: ReadBinding, previous: SeatResume, page: StreamPage): SeatResume {
  if (page.protocol_version !== binding.protocol_version || page.entries.length > 32 || (!page.entries.length && page.has_more)) throw new SessionFailure('incompatible');
  let next = previous;
  for (const entry of page.entries) {
    if (BigInt(entry.cursor) <= BigInt(next.next)) throw new SessionFailure('incompatible');
    switch (entry.publication.kind) {
      case 'view_replaced':
        checkObservation(binding, entry.publication.observation);
        next = { ...next, next: entry.cursor, state: { status: 'available', observation: entry.publication.observation } };
        break;
      case 'lifecycle': next = { ...next, next: entry.cursor, lifecycle: entry.publication.status }; break;
      case 'outcome': next = { ...next, next: entry.cursor, outcome: entry.publication.result }; break;
    }
  }
  if (page.next !== next.next) throw new SessionFailure('incompatible');
  return immutable(next);
}
