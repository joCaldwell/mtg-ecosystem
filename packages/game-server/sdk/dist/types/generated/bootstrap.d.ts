/* Generated from Rust mtg-server schema; do not edit. */

/**
 * Operator file; never log or derive Debug for capability-bearing data.
 */
export interface SeatBootstrap {
  bootstrap_version: number;
  capability: string;
  catalog_sha256: string;
  engine_build_id: string;
  match_id: string;
  protocol_version: number;
  server_build_id: string;
  server_origin: string;
  viewer: number;
}
