# Engine fixtures

The manifests in `catalogs/` select exact Archive R4 records. Regenerate expected
IR and digests together with `mtg-sim catalog build MANIFEST --write`; the
`mtg-compiler` unit tests check every manifest without writing.
`phase1-r4/` holds only records a manifest, witness or compiler test uses;
`provenance.json` and `accepted-archives.json` pin exactly those rows. Oracle
parser records live under `packages/oracle-parser/test/fixtures/`.
[Catalog commands](../../../../docs/game-engine-catalogs.md#catalog-source-and-commands)
describe provenance, extraction and verification. Removed reports, digests and
workloads remain in Git history; nothing here is a runtime diagnostic output.
