import type { CommandEnvelope } from '../generated/command.js';
import type { StreamPage, StreamEntry } from '../generated/stream.js';
import type { SubmissionResult } from '../generated/submission.js';
import type { CommandExecution } from '../generated/execution.js';
import type { PublicManifest } from '../generated/manifest.js';
import { command, commandIdentifier, counter, execution, manifest, stream, submission, ProtocolError } from './validation.ts';
import { strictJSON } from './strict-json.ts';
import { ROUTES } from '../generated/routes.ts';
export { ProtocolError, counter, PROTOCOL_VERSION } from './validation.ts';
export type { CommandEnvelope, PlayerResponse, ExactQuantity } from '../generated/command.js';
export type { StreamPage, StreamEntry, Observation, PlayerRequest, Publication, ExecutionStatus, RulesResult, DecimalU64Counter } from '../generated/stream.js';
export type { SubmissionResult, RejectionReason } from '../generated/submission.js';
export type { CommandExecution, CommandExecutionStatus } from '../generated/execution.js';
export type { PublicManifest, Catalog, Decklist, DeckEntry } from '../generated/manifest.js';

export class HttpError extends Error {
  readonly status: number;
  constructor(status: number) { super(`Game server returned HTTP ${status}`); this.name = 'HttpError'; this.status = status; }
}
export class TransportError extends Error {
  constructor() { super('Game server request failed; outcome may be unknown'); this.name = 'TransportError'; }
}
export interface ClientOptions {
  baseURL: string;
  matchId: string;
  capability: string;
  fetch?: typeof globalThis.fetch;
  maximumResponseBytes?: number;
}
export interface PollOptions { after: string; limit?: number; waitMs?: number; signal?: AbortSignal }
function aborted(): Error { const error = new Error('Request aborted; outcome may be unknown'); error.name = 'AbortError'; return error; }

/** One seat capability; no administrative or raw-state endpoints. */
export class GameClient {
  #base: string;
  #capability: string;
  #matchId: string;
  #fetch: typeof globalThis.fetch;
  #maximumResponseBytes: number;
  constructor(options: ClientOptions) {
    let url: URL;
    try { url = new URL(options.baseURL); } catch { throw new Error('Invalid base URL'); }
    const loopback = ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname);
    if ((url.protocol !== 'https:' && !(url.protocol === 'http:' && loopback)) || url.username || url.password || url.search || url.hash) {
      throw new Error('Expected HTTPS or local loopback base URL without credentials, query or fragment');
    }
    if (typeof options.matchId !== 'string' || options.matchId.length < 1 || options.matchId.length > 64 || /[^0-9a-f]/.test(options.matchId)) throw new Error('Invalid match identifier');
    if (typeof options.capability !== 'string' || options.capability.length < 1 || options.capability.length > 4096 || /[^\x21-\x7e]/.test(options.capability)) throw new Error('Invalid capability');
    const maximum = options.maximumResponseBytes ?? 8 * 1024 * 1024;
    if (!Number.isSafeInteger(maximum) || maximum < 1 || maximum > 32 * 1024 * 1024) throw new Error('Invalid response byte limit');
    this.#base = url.href.replace(/\/$/, '');
    this.#matchId = options.matchId;
    this.#capability = options.capability;
    this.#fetch = options.fetch ?? globalThis.fetch;
    this.#maximumResponseBytes = maximum;
  }
  async #request(operation: keyof typeof ROUTES, body: string | undefined, signal?: AbortSignal, parameters: { query?: URLSearchParams; commandId?: string } = {}): Promise<unknown> {
    const route = ROUTES[operation];
    let path = route.path.replace('{match_id}', this.#matchId);
    if (parameters.commandId !== undefined) path = path.replace('{command_id}', parameters.commandId);
    const suffix = parameters.query === undefined ? '' : `?${parameters.query}`;
    if (signal?.aborted) throw aborted();
    let response: Response;
    try {
      response = await this.#fetch(`${this.#base}${path}${suffix}`, {
        method: route.method, headers: { Authorization: `Bearer ${this.#capability}`, Accept: 'application/json', ...(body === undefined ? {} : { 'Content-Type': 'application/json' }) },
        ...(body === undefined ? {} : { body }), ...(signal === undefined ? {} : { signal }),
        redirect: 'error', credentials: 'omit', cache: 'no-store', referrerPolicy: 'no-referrer',
      });
    } catch { if (signal?.aborted) throw aborted(); throw new TransportError(); }
    const reject = async (error: Error): Promise<never> => {
      await response.body?.cancel().catch(() => {});
      throw error;
    };
    if (response.redirected) return reject(new ProtocolError('Unexpected redirected response'));
    if (!response.ok) return reject(new HttpError(response.status));
    if (!/^application\/json(?:\s*;|$)/i.test(response.headers.get('content-type') ?? '')) return reject(new ProtocolError('Expected JSON response'));
    const declared = Number(response.headers.get('content-length'));
    if (Number.isFinite(declared) && declared > this.#maximumResponseBytes) return reject(new ProtocolError('Response body exceeds limit'));
    const reader = response.body?.getReader();
    if (!reader) throw new ProtocolError('Missing JSON response body');
    let bytes = 0, text = '';
    const decoder = new TextDecoder('utf-8', { fatal: true });
    try {
      for (;;) {
        if (signal?.aborted) throw aborted();
        const result = await reader.read();
        if (result.done) break;
        bytes += result.value.byteLength;
        if (bytes > this.#maximumResponseBytes) throw new ProtocolError('Response body exceeds limit');
        text += decoder.decode(result.value, { stream: true });
      }
      text += decoder.decode();
      return strictJSON(text);
    } catch (error) {
      await reader.cancel().catch(() => {});
      if (signal?.aborted) throw aborted();
      if (error instanceof ProtocolError) throw error;
      throw new ProtocolError('Invalid JSON response');
    } finally { reader.releaseLock(); }
  }
  /** No retries: after an ambiguous transport error, caller reuses the exact envelope/id. */
  async submit(envelope: CommandEnvelope, options: { signal?: AbortSignal } = {}): Promise<SubmissionResult> {
    const validated = command(envelope);
    const commandId = validated.command_id;
    const body = JSON.stringify(validated);
    if (new TextEncoder().encode(body).byteLength > 65_536) throw new ProtocolError('Command body exceeds limit');
    const result = submission(await this.#request('submit_command', body, options.signal));
    if (result.command_id !== commandId) throw new ProtocolError('Submission result has a different command identifier');
    return result;
  }
  /** Public catalog/open decklists, without actual seed or private state. */
  async manifest(options: { signal?: AbortSignal } = {}): Promise<PublicManifest> {
    const result = manifest(await this.#request('get_manifest', undefined, options.signal));
    if (result.match_id !== this.#matchId) throw new ProtocolError('Public manifest has a different match identifier');
    return result;
  }
  /** Durable execution disposition for this authenticated seat's command ID. */
  async execution(id: string, options: { signal?: AbortSignal } = {}): Promise<CommandExecution> {
    const commandId = commandIdentifier(id);
    const result = execution(await this.#request('command_execution', undefined, options.signal, { commandId }));
    if (result.command_id !== commandId) throw new ProtocolError('Execution status has a different command identifier');
    return result;
  }
  /** Only cancels a still-queued command; accepted rules input is never undone. */
  async cancel(id: string, options: { signal?: AbortSignal } = {}): Promise<SubmissionResult> {
    const commandId = commandIdentifier(id);
    const result = submission(await this.#request('cancel_command', undefined, options.signal, { commandId }));
    if (result.command_id !== commandId) throw new ProtocolError('Cancellation result has a different command identifier');
    return result;
  }
  async poll(options: PollOptions): Promise<StreamPage> {
    const after = counter(options.after), limit = options.limit ?? 32, waitMs = options.waitMs ?? 20_000;
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 32 || !Number.isSafeInteger(waitMs) || waitMs < 0 || waitMs > 20_000) throw new Error('Invalid long-poll bounds');
    const query = new URLSearchParams({ after, limit: String(limit), wait_ms: String(waitMs) });
    const page = stream(await this.#request('poll_events', undefined, options.signal, { query }), after);
    if (page.entries.length > limit) throw new ProtocolError('Server exceeded requested page limit');
    return page;
  }
  /** Reconnect from the last entry the consumer processed; no hidden acknowledgment. */
  async *events(options: PollOptions): AsyncGenerator<StreamEntry, void, void> {
    let after = counter(options.after);
    for (;;) {
      const page = await this.poll({ ...options, after });
      for (const entry of page.entries) { yield entry; after = entry.cursor; }
    }
  }
}
