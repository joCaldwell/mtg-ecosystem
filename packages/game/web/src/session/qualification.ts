export interface ProductSessionIdentity {
  readonly profile: string;
  readonly catalog_sha256: string;
  readonly engine_build_id: string;
  readonly server_build_id: string;
}

// Only source-qualified product-browser evidence may add an exact identity.
// Standalone first-turn proof does not authorize the product's account route.
const approved: readonly ProductSessionIdentity[] = [{
  profile: "commander-foundations-v1",
  catalog_sha256: "ff307bd50c7714f3ad99c1bd558217cd38b0223c1fe48dfd200d2b611abc8b56",
  engine_build_id: "18d04f99c7f81c0c6169e4d060b5c83328ddaab2ba5535f8da74cbe5faf4aaac",
  server_build_id: "42b0f10f7082dfb62f7ebd59ac43e34c2ad0cc01d46d3c9361a2bda5fa1c33ae",
}];

/** Host composition only; configuration cannot grant browser qualification. */
export function isQualifiedProductSession(identity: ProductSessionIdentity | undefined): boolean {
  return !!identity && approved.some(value => value.profile === identity.profile
    && value.catalog_sha256 === identity.catalog_sha256
    && value.engine_build_id === identity.engine_build_id
    && value.server_build_id === identity.server_build_id);
}
