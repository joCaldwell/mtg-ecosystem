/** A real seat consumer for the finite pass/draw policy; no trusted engine APIs. */
import {
  GameClient, HttpError, ProtocolError, TransportError, PROTOCOL_VERSION,
  type CommandEnvelope, type Observation, type PlayerResponse,
  type PublicManifest, type RulesResult,
} from '../client.ts';

interface ConsumerLimits {
  acceptedInputs: number;
  durationMs: number;
  attempts: number;
  retryWindowMs: number;
  requestMs: number;
}
export const CONSUMER_LIMITS: Readonly<ConsumerLimits> = Object.freeze({
  acceptedInputs: 5_000, durationMs: 25 * 60_000,
  attempts: 24, retryWindowMs: 8 * 60_000, requestMs: 30_000,
});
export interface ConsumerBootstrap {
  baseURL: string;
  matchId: string;
  capability: string;
  viewer: number;
  outputPath?: string;
}
type Failure = 'bootstrap_invalid' | 'manifest_identity' | 'protocol_invalid'
  | 'unexpected_request' | 'invalid_candidates' | 'foreign_observation'
  | 'stale_revision' | 'invalid_response' | 'host_paused' | 'host_faulted'
  | 'command_cancelled' | 'retry_limit' | 'http_failure' | 'deadline'
  | 'input_limit' | 'cancelled' | 'consumer_failure';
class ConsumerFailure extends Error {
  readonly code: Failure;
  constructor(code: Failure) { super(code); this.code = code; }
}
export interface ConsumerCounts {
  commands_created: number;
  submission_attempts: number;
  accepted_inputs: number;
  rejected_inputs: number;
  poll_pages: number;
  publications: number;
  replacement_views: number;
  retries: number;
}
interface Summary {
  consumer_version: 1;
  viewer: number;
  counts: ConsumerCounts;
  cursor: string;
  elapsed_ms: number;
  engine_build_id?: string;
  catalog_sha256?: string;
}
export type ConsumerReport = Summary & (
  { status: 'completed'; result: RulesResult } |
  { status: 'stopped'; reason: Failure }
);
/** Test seams change transport/time, never supply an authoritative Game. */
export interface ConsumerOptions {
  expectedProfile?: 'commander-foundations-v1' | 'commander-attachments-v1';
  policy?: (observation: Observation, viewer: number, manifest: PublicManifest) => PlayerResponse | null;
  fetch?: typeof globalThis.fetch;
  now?: () => number;
  sleep?: (milliseconds: number) => Promise<void>;
  signal?: AbortSignal;
  limits?: Partial<ConsumerLimits>;
}

export function parseBootstrap(value: unknown): ConsumerBootstrap {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new ConsumerFailure('bootstrap_invalid');
  const row = value as Record<string, unknown>;
  if (Object.keys(row).some(key => !['baseURL', 'matchId', 'capability', 'viewer', 'outputPath'].includes(key))
    || typeof row.baseURL !== 'string' || typeof row.matchId !== 'string'
    || typeof row.capability !== 'string' || row.capability.length !== 64 || /[^0-9a-f]/.test(row.capability)
    || typeof row.viewer !== 'number' || !Number.isInteger(row.viewer) || row.viewer < 0 || row.viewer > 3
    || (row.outputPath !== undefined && (typeof row.outputPath !== 'string' || row.outputPath.length < 1 || row.outputPath.length > 4096 || row.outputPath.includes('\0')))) {
    throw new ConsumerFailure('bootstrap_invalid');
  }
  try { new GameClient({ baseURL: row.baseURL, matchId: row.matchId, capability: row.capability }); }
  catch { throw new ConsumerFailure('bootstrap_invalid'); }
  // A bootstrap must not put its bearer into a URL or public match identifier.
  if (row.baseURL.includes(row.capability) || row.matchId.includes(row.capability)) throw new ConsumerFailure('bootstrap_invalid');
  return {
    baseURL: row.baseURL, matchId: row.matchId, capability: row.capability, viewer: row.viewer,
    ...(row.outputPath === undefined ? {} : { outputPath: row.outputPath as string }),
  };
}

/** Only offered handles and counts enter responses. No cast, land or concession policy. */
export function foundationsPolicy(observation: Observation, viewer: number): PlayerResponse | null {
  if (observation.view.viewer !== viewer) throw new ConsumerFailure('foreign_observation');
  const request = observation.decision;
  if (request === null) return null;
  if (request.player !== viewer) throw new ConsumerFailure('foreign_observation');
  switch (request.request) {
    case 'mulligan': return { response: 'mulligan', take: false };
    case 'priority': return { response: 'pass' };
    case 'attackers': return { response: 'attackers', attacks: [] };
    case 'blockers': return { response: 'blockers', blocks: [] };
    case 'commander_return': return { response: 'commander_return', to_command: true };
    case 'discard':
      if (!Number.isSafeInteger(request.count) || request.count < 0 || request.count > request.candidates.length
        || new Set(request.candidates).size !== request.candidates.length) throw new ConsumerFailure('invalid_candidates');
      return { response: 'discard', objects: request.candidates.slice(0, request.count) };
    default: throw new ConsumerFailure('unexpected_request');
  }
}

function canonical(value: unknown): string {
  // Same compact JSON contract as Rust contracts::canonical: ordered arrays,
  // UTF-8 lexical object keys and exact integer numbers. Wire decoding has
  // already rejected duplicate keys, non-scalar strings and numeric aliases.
  const encoder = new TextEncoder();
  const compare = (a: string, b: string): number => {
    const aa = encoder.encode(a), bb = encoder.encode(b);
    for (let i = 0; i < Math.min(aa.length, bb.length); i++) if (aa[i] !== bb[i]) return aa[i]! - bb[i]!;
    return aa.length - bb.length;
  };
  const write = (v: unknown): string => {
    if (v === null || typeof v === 'string' || typeof v === 'boolean') return JSON.stringify(v);
    if (typeof v === 'number' && Number.isSafeInteger(v) && !Object.is(v, -0)) return JSON.stringify(v);
    if (Array.isArray(v)) return `[${v.map(write).join(',')}]`;
    if (v && typeof v === 'object') return `{${Object.keys(v).sort(compare).map(key => `${JSON.stringify(key)}:${write((v as Record<string, unknown>)[key])}`).join(',')}}`;
    throw new ConsumerFailure('manifest_identity');
  };
  return write(value);
}
async function checkManifest(value: PublicManifest, matchId: string, expectedProfile: string): Promise<string> {
  const hex = (text: string): boolean => text.length === 64 && !/[^0-9a-f]/.test(text);
  if (value.match_id !== matchId || value.protocol_version !== PROTOCOL_VERSION
    || value.profile !== expectedProfile || !hex(value.engine_build_id) || !hex(value.server_build_id) || !hex(value.catalog_sha256)
    || !Number.isInteger(value.starting_player) || value.starting_player < 0 || value.starting_player > 3) throw new ConsumerFailure('manifest_identity');
  const hash = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(canonical(value.catalog)));
  const actual = Array.from(new Uint8Array(hash), byte => byte.toString(16).padStart(2, '0')).join('');
  if (actual !== value.catalog_sha256) throw new ConsumerFailure('manifest_identity');
  return canonical(value);
}
function identifier(viewer: number, ordinal: number): string {
  return viewer.toString(16).padStart(16, '0') + ordinal.toString(16).padStart(16, '0');
}

export async function runFoundationsConsumer(input: ConsumerBootstrap, options: ConsumerOptions = {}): Promise<ConsumerReport> {
  const now = options.now ?? (() => performance.now());
  const started = now();
  const sleep = options.sleep ?? (milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds)));
  const limits = { ...CONSUMER_LIMITS, ...options.limits };
  const expectedProfile = options.expectedProfile ?? 'commander-foundations-v1';
  const counts: ConsumerCounts = { commands_created: 0, submission_attempts: 0, accepted_inputs: 0, rejected_inputs: 0, poll_pages: 0, publications: 0, replacement_views: 0, retries: 0 };
  let cursor = '0', manifest: PublicManifest | undefined;
  const summary = (): Summary => ({
    consumer_version: 1, viewer: input.viewer, counts, cursor,
    elapsed_ms: Math.max(0, Math.floor(now() - started)),
    ...(manifest === undefined ? {} : { engine_build_id: manifest.engine_build_id, catalog_sha256: manifest.catalog_sha256 }),
  });
  const active = (): void => {
    if (options.signal?.aborted) throw new ConsumerFailure('cancelled');
    if (now() - started >= limits.durationMs) throw new ConsumerFailure('deadline');
  };
  async function request<T>(operation: (signal: AbortSignal) => Promise<T>): Promise<T> {
    const until = Math.min(started + limits.durationMs, now() + limits.retryWindowMs);
    for (let attempt = 0; attempt < limits.attempts; attempt++) {
      active();
      if (now() >= until) throw new ConsumerFailure('retry_limit');
      const controller = new AbortController();
      const relay = (): void => controller.abort();
      options.signal?.addEventListener('abort', relay, { once: true });
      const timer = setTimeout(relay, Math.max(1, Math.min(limits.requestMs, until - now())));
      try {
        const result = await operation(controller.signal);
        active();
        return result;
      }
      catch (error) {
        active();
        const transient = error instanceof TransportError
          || (error instanceof HttpError && [429, 503].includes(error.status))
          || (error instanceof Error && error.name === 'AbortError');
        if (!transient) {
          if (error instanceof ProtocolError) throw new ConsumerFailure('protocol_invalid');
          if (error instanceof HttpError) throw new ConsumerFailure('http_failure');
          throw new ConsumerFailure('consumer_failure');
        }
        if (attempt + 1 >= limits.attempts || now() >= until) throw new ConsumerFailure('retry_limit');
        counts.retries++;
      } finally {
        clearTimeout(timer);
        options.signal?.removeEventListener('abort', relay);
      }
      await sleep(Math.min(1_000, 100 * 2 ** Math.min(attempt, 4), Math.max(0, until - now())));
    }
    throw new ConsumerFailure('retry_limit');
  }
  try {
    const bootstrap = parseBootstrap(input);
    for (const key of Object.keys(CONSUMER_LIMITS) as (keyof typeof CONSUMER_LIMITS)[]) {
      if (!Number.isSafeInteger(limits[key]) || limits[key] < 1 || limits[key] > CONSUMER_LIMITS[key]) throw new ConsumerFailure('bootstrap_invalid');
    }
    const client = new GameClient({ baseURL: bootstrap.baseURL, matchId: bootstrap.matchId, capability: bootstrap.capability, ...(options.fetch === undefined ? {} : { fetch: options.fetch }) });
    const first = await request(signal => client.manifest({ signal }));
    const pinned = await checkManifest(first, bootstrap.matchId, expectedProfile);
    active();
    if ([first.engine_build_id, first.catalog_sha256].some(value => value.includes(bootstrap.capability))) throw new ConsumerFailure('manifest_identity');
    manifest = first;
    let observation: Observation | undefined, outcome: RulesResult | undefined;
    let noFurtherCommands = false;
    const answered = new Set<string>();
    for (;;) {
      active();
      let more: boolean;
      do {
        const page = await request(signal => client.poll({ after: cursor, limit: 32, waitMs: 20_000, signal }));
        counts.poll_pages++;
        for (const entry of page.entries) {
          counts.publications++;
          switch (entry.publication.kind) {
            case 'view_replaced':
              observation = entry.publication.observation;
              if (observation.view.viewer !== bootstrap.viewer || (observation.decision !== null && observation.decision.player !== bootstrap.viewer)) throw new ConsumerFailure('foreign_observation');
              counts.replacement_views++;
              break;
            case 'outcome':
              if (entry.publication.result.outcome === 'win' && (entry.publication.result.player < 0 || entry.publication.result.player > 3)) throw new ConsumerFailure('protocol_invalid');
              if (outcome !== undefined && canonical(outcome) !== canonical(entry.publication.result)) throw new ConsumerFailure('protocol_invalid');
              outcome = entry.publication.result;
              break;
            case 'lifecycle':
              switch (entry.publication.status.status) {
                case 'resource_paused': throw new ConsumerFailure('host_paused');
                case 'faulted': throw new ConsumerFailure('host_faulted');
                case 'cancelled': throw new ConsumerFailure('command_cancelled');
                case 'rejected': throw new ConsumerFailure('invalid_response');
                case 'finished': noFurtherCommands = true; break;
                default: break;
              }
              break;
          }
        }
        cursor = page.next;
        more = page.has_more;
      } while (more);
      if (outcome !== undefined) {
        const final = await request(signal => client.manifest({ signal }));
        if (await checkManifest(final, bootstrap.matchId, expectedProfile) !== pinned) throw new ConsumerFailure('manifest_identity');
        active();
        return { ...summary(), status: 'completed', result: outcome };
      }
      if (observation === undefined || observation.decision === null || noFurtherCommands) continue;
      const token = observation.decision.token;
      if (answered.has(token)) continue;
      if (counts.accepted_inputs >= limits.acceptedInputs) throw new ConsumerFailure('input_limit');
      const response = (options.policy ?? foundationsPolicy)(observation, bootstrap.viewer, manifest);
      if (response === null) throw new ConsumerFailure('protocol_invalid');
      counts.commands_created++;
      const envelope: CommandEnvelope = {
        protocol_version: PROTOCOL_VERSION, command_id: identifier(bootstrap.viewer, counts.commands_created),
        expected_revision: cursor, command: { kind: 'decision', token, response },
      };
      // This exact object/body/ID is retained across ambiguous retries; no poll,
      // token rebinding or new ID occurs until this submission has an outcome.
      answered.add(token);
      const result = await request(signal => {
        counts.submission_attempts++;
        return client.submit(envelope, { signal });
      });
      if (result.status === 'accepted') counts.accepted_inputs++;
      else {
        counts.rejected_inputs++;
        switch (result.reason) {
          case 'already_finished': noFurtherCommands = true; break;
          case 'stale_revision': throw new ConsumerFailure('stale_revision');
          case 'resource_paused': throw new ConsumerFailure('host_paused');
          case 'cancelled': throw new ConsumerFailure('command_cancelled');
          case 'invalid_response': throw new ConsumerFailure('invalid_response');
        }
      }
    }
  } catch (error) {
    return { ...summary(), status: 'stopped', reason: error instanceof ConsumerFailure ? error.code : 'consumer_failure' };
  }
}
