/* Generated from Rust mtg-server schema; do not edit. */

/**
 * Artifact location/integrity only; the offline producer owns its JSON shape.
 */
export interface PresentationDescriptor {
  catalog_sha256: string;
  path: string;
  presentation_schema_sha256: string;
  presentation_sha256: string;
  presentation_version: number;
  producer_sha256: string;
}
