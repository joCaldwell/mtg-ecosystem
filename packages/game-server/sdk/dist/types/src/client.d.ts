import type { ActionQueryRequest } from '../generated/action_request.js';
import type { ActionQueryResponse } from '../generated/action_query.js';
import type { CommandEnvelope } from '../generated/command.js';
import type { StreamPage, StreamEntry } from '../generated/stream.js';
import type { SubmissionResult } from '../generated/submission.js';
import type { CommandExecution } from '../generated/execution.js';
import type { PublicManifest } from '../generated/manifest.js';
import type { SeatResume } from '../generated/resume.js';
import type { SeatBootstrap } from '../generated/bootstrap.js';
import type { PresentationDescriptor, PresentationArtifact } from './public.js';
export { parseClientConfig, readClientConfig } from './public.js';
export type { ClientConfig, PresentationDescriptor, PresentationArtifact } from './public.js';
export { HttpError, TransportError } from './transport.js';
export { ProtocolError, counter, PROTOCOL_VERSION } from './validation.js';
export type { CommandEnvelope, PlayerResponse, ExactQuantity } from '../generated/command.js';
export type { StreamPage, StreamEntry, Observation, DecisionContext, DecisionSource, PlayerRequest, Publication, ExecutionStatus, RulesResult, DecimalU64Counter } from '../generated/stream.js';
export type { SubmissionResult, RejectionReason } from '../generated/submission.js';
export type { CommandExecution, CommandExecutionStatus } from '../generated/execution.js';
export type { PublicManifest, Catalog, Decklist, DeckEntry } from '../generated/manifest.js';
export type { SeatResume, ResumeState } from '../generated/resume.js';
export type { SeatBootstrap } from '../generated/bootstrap.js';
export type { ActionQueryRequest } from '../generated/action_request.js';
export type { ActionQueryResponse, ActionQueryStatus, ActionQueryUnavailable, ActionPage, ActionBeginning, ActionInitiation, ActionScope, ActionLabel, ActionDecisionContext, ActionSourceContext, ActionUnavailable } from '../generated/action_query.js';
/** Parse only an explicitly imported single-seat file; never persist or log it. */
export declare function parseSeatBootstrap(text: string): SeatBootstrap;
/** Detached immutable journal input; exactBody is the authoritative wire string. */
export interface PreparedCommand {
    readonly envelope: CommandEnvelope;
    readonly exactBody: string;
}
/** Recovery validates the original bytes without canonicalizing or reserializing. */
export declare function parsePreparedCommand(exactBody: string): PreparedCommand;
/** Snapshot, validate and detach from mutable caller-owned input before journaling. */
export declare function prepareCommand(envelope: CommandEnvelope): PreparedCommand;
export interface ClientOptions {
    baseURL: string;
    matchId: string;
    capability: string;
    fetch?: typeof globalThis.fetch;
    maximumResponseBytes?: number;
}
export interface PollOptions {
    after: string;
    limit?: number;
    waitMs?: number;
    signal?: AbortSignal;
}
/** One seat capability; no administrative or raw-state endpoints. */
export declare class GameClient {
    #private;
    constructor(options: ClientOptions);
    /** Public immutable metadata only. 404 is explicit; invalid data fails closed. */
    presentation(options?: {
        signal?: AbortSignal;
    }): Promise<{
        descriptor: PresentationDescriptor;
        artifact: PresentationArtifact;
    }>;
    /** No retries: after an ambiguous transport error, caller reuses the exact envelope/id. */
    submit(envelope: CommandEnvelope, options?: {
        signal?: AbortSignal;
    }): Promise<SubmissionResult>;
    /** Send one validated journal body verbatim. Caller owns every retry/recovery decision. */
    submitExact(exactBody: string, options?: {
        signal?: AbortSignal;
    }): Promise<SubmissionResult>;
    /** Public catalog/open decklists, without actual seed or private state. */
    manifest(options?: {
        signal?: AbortSignal;
    }): Promise<PublicManifest>;
    /** One committed boundary; caller binds viewer/build/catalog and polls after next. */
    resume(options?: {
        signal?: AbortSignal;
    }): Promise<SeatResume>;
    /** Durable execution disposition for this authenticated seat's command ID. */
    execution(id: string, options?: {
        signal?: AbortSignal;
    }): Promise<CommandExecution>;
    /** Only cancels a still-queued command; accepted rules input is never undone. */
    cancel(id: string, options?: {
        signal?: AbortSignal;
    }): Promise<SubmissionResult>;
    /** One bounded pure discovery/cursor/cancel operation. No retry or automatic pagination. */
    queryActions(request: ActionQueryRequest, options?: {
        signal?: AbortSignal;
    }): Promise<ActionQueryResponse>;
    poll(options: PollOptions): Promise<StreamPage>;
    /** Reconnect from the last entry the consumer processed; no hidden acknowledgment. */
    events(options: PollOptions): AsyncGenerator<StreamEntry, void, void>;
}
