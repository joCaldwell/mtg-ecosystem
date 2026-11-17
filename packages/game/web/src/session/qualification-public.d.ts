export interface ProductSessionIdentity {
  readonly profile: string;
  readonly catalog_sha256: string;
  readonly engine_build_id: string;
  readonly server_build_id: string;
}
/** Exact native identities qualified through the real product browser route. */
export declare function isQualifiedProductSession(identity: ProductSessionIdentity | undefined): boolean;
