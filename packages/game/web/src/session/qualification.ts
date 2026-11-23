export interface ProductSessionIdentity {
  readonly profile: string;
  readonly catalog_sha256: string;
  readonly engine_build_id: string;
  readonly server_build_id: string;
}

// Add a build identity only after the current SDK and product browser have
// completed the hosted Foundations proof together.
const approved: readonly ProductSessionIdentity[] = [];

/** Host composition only; configuration cannot grant browser qualification. */
export function isQualifiedProductSession(identity: ProductSessionIdentity | undefined): boolean {
  return !!identity && approved.some(value => value.profile === identity.profile
    && value.catalog_sha256 === identity.catalog_sha256
    && value.engine_build_id === identity.engine_build_id
    && value.server_build_id === identity.server_build_id);
}
