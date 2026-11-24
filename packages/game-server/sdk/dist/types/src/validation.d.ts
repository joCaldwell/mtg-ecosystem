import type { CommandEnvelope } from '../generated/command.js';
import type { StreamPage } from '../generated/stream.js';
import type { SubmissionResult } from '../generated/submission.js';
import type { CommandExecution } from '../generated/execution.js';
import type { PublicManifest } from '../generated/manifest.js';
import type { SeatResume } from '../generated/resume.js';
import type { SeatBootstrap } from '../generated/bootstrap.js';
export { PROTOCOL_VERSION } from '../generated/protocol.js';
export declare class ProtocolError extends Error {
    constructor(message?: string);
}
export declare function counter(value: string): string;
export declare function commandIdentifier(value: string): string;
export declare function manifest(value: unknown): PublicManifest;
export declare function resume(value: unknown): SeatResume;
export declare function bootstrap(value: unknown): SeatBootstrap;
export declare function execution(value: unknown): CommandExecution;
export declare function command(value: unknown): CommandEnvelope;
export declare function submission(value: unknown): SubmissionResult;
export declare function stream(value: unknown, after: string): StreamPage;
