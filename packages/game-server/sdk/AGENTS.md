# Generated protocol SDK

Read the parent server manual and this package's README. Work here does not
establish that HTTP hosting, durable recovery or four-player integration passes.
Rust protocol/public core DTOs are authoritative; do not add manual replacements
for PlayerResponse, Observation, tuples, quantities or transport counters.

This is a nested isolated npm package, not a root workspace. Its node_modules
must be a real local directory; do not install into or replace the root shared
node_modules symlink. Keep this package's exact dependency lockfile.

From repository root:

```sh
npm --prefix packages/game-server/sdk ci --workspaces=false --ignore-scripts
MTG_SERVER_BIN=/absolute/path/to/mtg-server npm --prefix packages/game-server/sdk run verify --workspaces=false
```

The Rust owner builds mtg-server separately. Generation never starts Cargo or
changes Rust source. The verified workflow exports Rust Draft7 schemas,
generates precise declarations, runs strict TypeScript including an exhaustive
response consumer/tuple negatives, runs the Node transport/generator tests,
and writes generated/verification.json + verification.log. Schemas/types/logs
remain ignored. Node26.11.1 is the checked native TypeScript test environment.

`MTG_SERVER_SCHEMA` is an explicit export-file input for isolated generator
verification; preserve its source provenance separately. `MTG_SERVER_OUTPUT`
is a generator-test override. Verification itself uses this package's ordinary
ignored generated directory so type/runtime checks read the same outputs.

No automatic command retries or fabricated actions. Capabilities belong only
in authorization headers/private fields, never URLs/errors/JSON logs. Enforce
cursor and body bounds, reject extra fields and imprecise numeric data, and
cancel response streams on early rejection. Mock transport tests are necessary
but do not replace actual authenticated HTTP/crash/reconnect/isolation tests.

Browser consumers import the package root or `/browser`, after explicit
`build:browser`. Generation emits standalone Ajv validators; never restore
runtime compilation or relax CSP. `verify` also builds the browser package and
checks its declarations/exports. `test:browser` separately runs the production
bundle in pinned Chromium and Firefox under restrictive CSP, against a synthetic
HTTP fixture. Install these browsers explicitly with the local Playwright CLI;
checks never download them or start Cargo. A real host proof is separately gated
on the native owner's exact compatible executable and one-seat bootstrap.
