import type { ActionQueryRequest } from '../generated/action_request.js';
import type { ActionQueryResponse } from '../generated/action_query.js';
import type { CommandEnvelope } from '../generated/command.js';
import type { StreamPage, StreamEntry } from '../generated/stream.js';
import type { SubmissionResult } from '../generated/submission.js';
import type { CommandExecution } from '../generated/execution.js';
import type { PublicManifest } from '../generated/manifest.js';
import type { SeatResume } from '../generated/resume.js';
import type { SeatBootstrap } from '../generated/bootstrap.js';
import { actionRequest, actionQuery, command, commandIdentifier, counter, execution, manifest, stream, submission, resume, bootstrap, ProtocolError } from './validation.ts';
import { strictJSON } from './strict-json.ts';
import { requestJSON } from './transport.ts';
import { presentationDescriptor, presentationArtifact } from './public.ts';
import type { PresentationDescriptor, PresentationArtifact } from './public.ts';
export { parseClientConfig, readClientConfig } from './public.ts';
export type { ClientConfig, PresentationDescriptor, PresentationArtifact } from './public.ts';
export { HttpError, TransportError } from './transport.ts';
import { ROUTES } from '../generated/routes.ts';
export { ProtocolError, counter, PROTOCOL_VERSION } from './validation.ts';
export type { CommandEnvelope, PlayerResponse, ExactQuantity } from '../generated/command.js';
export type { StreamPage, StreamEntry, Observation, PlayerRequest, Publication, ExecutionStatus, RulesResult, DecimalU64Counter } from '../generated/stream.js';
export type { SubmissionResult, RejectionReason } from '../generated/submission.js';
export type { CommandExecution, CommandExecutionStatus } from '../generated/execution.js';
export type { PublicManifest, Catalog, Decklist, DeckEntry } from '../generated/manifest.js';
export type { SeatResume, ResumeState } from '../generated/resume.js';
export type { SeatBootstrap } from '../generated/bootstrap.js';

export type { ActionQueryRequest } from '../generated/action_request.js';
export type { ActionQueryResponse, ActionQueryStatus, ActionQueryUnavailable, ActionPage, ActionBeginning, ActionInitiation, ActionScope, ActionLabel, ActionDecisionContext, ActionSourceContext, ActionUnavailable } from '../generated/action_query.js';

/** Parse only an explicitly imported single-seat file; never persist or log it. */
export function parseSeatBootstrap(text: string): SeatBootstrap {
  if (new TextEncoder().encode(text).byteLength > 4096) throw new ProtocolError('Seat bootstrap exceeds limit');
  return bootstrap(strictJSON(text));
}

/** Detached immutable journal input; exactBody is the authoritative wire string. */
export interface PreparedCommand { readonly envelope: CommandEnvelope; readonly exactBody: string }
function freezeCommand<T>(value:T):T {
  if (value && typeof value === 'object') { for (const child of Object.values(value)) freezeCommand(child); Object.freeze(value); }
  return value;
}
/** Recovery validates the original bytes without canonicalizing or reserializing. */
export function parsePreparedCommand(exactBody:string):PreparedCommand {
  if (typeof exactBody !== 'string' || new TextEncoder().encode(exactBody).byteLength > 65_536) throw new ProtocolError('Command body exceeds limit');
  return Object.freeze({ envelope:freezeCommand(command(strictJSON(exactBody))), exactBody });
}
/** Snapshot, validate and detach from mutable caller-owned input before journaling. */
export function prepareCommand(envelope:CommandEnvelope):PreparedCommand {
  command(envelope);
  return parsePreparedCommand(JSON.stringify(envelope));
}

export interface ClientOptions {
  baseURL: string;
  matchId: string;
  capability: string;
  fetch?: typeof globalThis.fetch;
  maximumResponseBytes?: number;
}
export interface PollOptions { after: string; limit?: number; waitMs?: number; signal?: AbortSignal }
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
    // Window.fetch requires its browser-global receiver. Calling an unbound
    // default as a private instance method fails in browsers (Node permits it).
    this.#fetch = (options.fetch ?? globalThis.fetch).bind(globalThis);
    this.#maximumResponseBytes = maximum;
  }
  async #request(operation: keyof typeof ROUTES, body: string | undefined, signal?: AbortSignal, parameters: { query?: URLSearchParams; commandId?: string; maximumBytes?: number } = {}): Promise<unknown> {
    const maximumBytes = Math.min(this.#maximumResponseBytes, parameters.maximumBytes ?? this.#maximumResponseBytes);
    const route = ROUTES[operation];
    let path = route.path.replace('{match_id}', this.#matchId);
    if (parameters.commandId !== undefined) path = path.replace('{command_id}', parameters.commandId);
    const suffix = parameters.query === undefined ? '' : `?${parameters.query}`;
    return (await requestJSON(this.#fetch, `${this.#base}${path}${suffix}`, {
      method: route.method, headers: { Authorization: `Bearer ${this.#capability}`, Accept: 'application/json', ...(body === undefined ? {} : {'Content-Type':'application/json'}) },
      ...(body === undefined ? {} : {body}),
    }, maximumBytes, signal)).value;
  }
  /** Public immutable metadata only. 404 is explicit; invalid data fails closed. */
  async presentation(options:{signal?:AbortSignal}={}):Promise<{descriptor:PresentationDescriptor;artifact:PresentationArtifact}> {
    const descriptor=presentationDescriptor(await this.#request('get_presentation',undefined,options.signal,{maximumBytes:4096}));
    const {value,bytes}=await requestJSON(this.#fetch, `${this.#base}${descriptor.path}`, {method:'GET',headers:{Accept:'application/json'}},Math.min(this.#maximumResponseBytes,4*1024*1024),options.signal);
    return {descriptor,artifact:await presentationArtifact(value,bytes,descriptor)};
  }
  /** No retries: after an ambiguous transport error, caller reuses the exact envelope/id. */
  async submit(envelope: CommandEnvelope, options: { signal?: AbortSignal } = {}): Promise<SubmissionResult> {
    return this.submitExact(prepareCommand(envelope).exactBody, options);
  }
  /** Send one validated journal body verbatim. Caller owns every retry/recovery decision. */
  async submitExact(exactBody:string, options:{signal?:AbortSignal}={}):Promise<SubmissionResult> {
    const {envelope, exactBody:body}=parsePreparedCommand(exactBody);
    const commandId=envelope.command_id;
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
  /** One committed boundary; caller binds viewer/build/catalog and polls after next. */
  async resume(options: { signal?: AbortSignal } = {}): Promise<SeatResume> {
    const result = resume(await this.#request('get_resume', undefined, options.signal, { maximumBytes: 131_072 }));
    if (result.match_id !== this.#matchId) throw new ProtocolError('Seat resume has a different match identifier');
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
  /** One bounded pure discovery/cursor/cancel operation. No retry or automatic pagination. */
  async queryActions(request:ActionQueryRequest, options:{signal?:AbortSignal}={}):Promise<ActionQueryResponse> {
    const validated=actionRequest(request);
    const body=JSON.stringify(validated);
    if(new TextEncoder().encode(body).byteLength>2048) throw new ProtocolError('Action query body exceeds limit');
    const result=actionQuery(await this.#request('query_actions',body,options.signal,{maximumBytes:65_536}));
    if(result.match_id!==this.#matchId) throw new ProtocolError('Action query has a different match identifier');
    if(validated.operation==='start' && (result.outcome.status==='complete' || result.outcome.status==='incomplete')) {
      const page=result.outcome.page;
      if(page.token!==validated.token || page.scope.scope!==validated.scope.scope || (page.scope.scope==='object' && validated.scope.scope==='object' && page.scope.object!==validated.scope.object) || page.beginnings.length>validated.page_size) throw new ProtocolError('Action query page has a different request binding');
    }
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
