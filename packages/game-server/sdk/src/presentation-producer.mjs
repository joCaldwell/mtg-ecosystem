// Node-only public bridge to the ORIGINAL source producer. The browser entry
// never imports this module or compiler-pinned catalog/source JSON.
import { canonical, presentation, sha256 } from '../../tools/client-artifacts.ts';
import { manifest, ProtocolError } from './validation.ts';
import { strictJSON } from './strict-json.ts';
import { presentationDescriptor, presentationArtifact } from './public.ts';
import { PRESENTATION_SCHEMA_SHA256, PRESENTATION_PRODUCER_SHA256 } from '../generated/presentation-identity.ts';

/** Produce only public catalog facts after exact native identity/source checks. */
export async function producePresentation(nativePublicManifest, expected) {
  const encoded = JSON.stringify(nativePublicManifest);
  if (typeof encoded !== 'string' || new TextEncoder().encode(encoded).length > 8 * 1024 * 1024) throw new ProtocolError('Public manifest exceeds limit');
  // Detach caller objects; strict parsing also rejects invalid Unicode/numerics.
  const value = manifest(strictJSON(encoded));
  for (const key of ['match_id', 'engine_build_id', 'server_build_id', 'catalog_sha256', 'profile']) {
    if (typeof expected?.[key] !== 'string' || value[key] !== expected[key]) throw new ProtocolError('Presentation native identity mismatch');
  }
  if (sha256(canonical(value.catalog)) !== value.catalog_sha256) throw new ProtocolError('Presentation catalog digest mismatch');
  // Source JSON is opaque to the browser. Reject ambiguous source encodings
  // before the original producer selects its exact allowlisted descriptive data.
  for (const definition of value.catalog.definitions) strictJSON(definition.provenance.source_json);
  let artifact;
  try { artifact = presentation(value.catalog); }
  catch { throw new ProtocolError('Invalid presentation source provenance'); }
  const bytes = new TextEncoder().encode(canonical(artifact) + '\n');
  if (bytes.length > 4 * 1024 * 1024) throw new ProtocolError('Presentation artifact exceeds limit');
  const hash = sha256(bytes);
  const descriptor = presentationDescriptor({ presentation_version: 1, catalog_sha256: value.catalog_sha256,
    presentation_sha256: hash, presentation_schema_sha256: PRESENTATION_SCHEMA_SHA256,
    producer_sha256: PRESENTATION_PRODUCER_SHA256, path: `/presentation/${hash}.json` });
  await presentationArtifact(artifact, bytes, descriptor);
  if (new TextEncoder().encode(JSON.stringify(descriptor)).length > 4096) throw new ProtocolError('Presentation descriptor exceeds limit');
  return { descriptor: Object.freeze(descriptor), bytes };
}
