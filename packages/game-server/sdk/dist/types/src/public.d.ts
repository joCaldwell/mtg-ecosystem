import type { ClientConfig } from '../generated/client_config.js';
import type { PresentationDescriptor } from '../generated/presentation_descriptor.js';
import type { PresentationArtifact } from '../generated/presentation.js';
export type { ClientConfig, PresentationDescriptor, PresentationArtifact };
export declare function parseClientConfig(text: string): ClientConfig;
export declare function readClientConfig(baseURL: string, options?: {
    signal?: AbortSignal;
    fetch?: typeof fetch;
}): Promise<ClientConfig>;
export declare function presentationDescriptor(value: unknown): PresentationDescriptor;
export declare function presentationArtifact(value: unknown, bytes: Uint8Array, descriptor: PresentationDescriptor): Promise<PresentationArtifact>;
