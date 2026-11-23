import type { PublicManifest } from './types/generated/manifest.js';
import type { PresentationDescriptor } from './types/generated/presentation_descriptor.js';
export declare function producePresentation(nativePublicManifest: PublicManifest, expected: Pick<PublicManifest, 'match_id' | 'engine_build_id' | 'server_build_id' | 'catalog_sha256' | 'profile'>): Promise<{ descriptor: Readonly<PresentationDescriptor>; bytes: Uint8Array }>;
