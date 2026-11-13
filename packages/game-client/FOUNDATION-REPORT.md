# Client foundation evidence

Implementation base: `691496f6410c188d0e26f3a6f46d35f5ee159243` (reviewed plans
already merged). Authored paths: `packages/game-client/` only. Peer prerequisites
were cherry-picked unchanged, with original commit attribution. No root package
change, Cargo invocation, hosted-profile change, deployment or PR merge.

## Source-backed fixture checkpoint

The current UI consumes the native lane's exact authenticated HTTP seat-zero
opening observation and producer-owned public presentation allowlist. It shows
seven separately handled copies in the own hand, all four public commanders,
source text, current observed characteristics and a read-only mulligan inspector.
It makes no API requests. Join remains visibly disconnected and collects nothing.
The missing-metadata route intentionally omits metadata from the same observation;
current characteristics and identity survive, without guessed printed facts.

Dependency commits (source-qualified receipts remain in the peer reports):

| Authority | Original commit | Local unchanged cherry-pick |
| --- | --- | --- |
| Native resume/bootstrap/presentation | `5ff4fd94b6dace1cdee0f0223305e054c42af8f3` | `ccb6d88` |
| SDK browser exports/standalone validation | `f2fb12910bcc4a4d52d6c88b98f61647f4246b24` | `b161ae3` |
| SDK frozen resume/bootstrap adaptation | `2fd07521cc8f342defe05b02b0cb2e00dde226b2` | `fce564c` |

Client shell checkpoint: `1b7eb63`. The fixture checkpoint is the commit containing
this report update. Native/SDK prerequisites are included in this branch; their
implementation remains owned by [PR #14](https://github.com/joCaldwell/mtg-ecosystem/pull/14)
and [PR #13](https://github.com/joCaldwell/mtg-ecosystem/pull/13).

Exact identities:

| Artifact | SHA-256 |
| --- | --- |
| Protocol schema | `519ce989ecc98c87e4aba0a14c1c7aa04fffa1aa552c7f2905cdb42f290b93ae` |
| Bootstrap schema | `0655664d30126489890dad1bca3b2e3b68a0e62f982c3f42be2e2aebf687315a` |
| Catalog | `fc4bd3390e63ea352e129ac370fa848dbee11cf0700e7e6781d449fd49244a7f` |
| HTTP opening fixture | `cb0214f247ccb0c651f6cb97534539420e0e1a3eb18867dd75509742335d87d9` |
| Public presentation | `f3d1c0c33a76565ab107c0201320072c705845cadc4508c7364f7c0b3d3b16b7` |
| Producer presentation schema | `a7045da3459cd5b6a126ca804ea22716062c63ce3127017bb5f3637c840231a6` |
| Presentation producer source | `ac83036695a5e17810c381e55b47cf6a7338585e824709228bf434cb897ef5fd` |

`fixtures/foundations-opening/provenance.json` pins all copied bytes. The native
producer schema generates ignored presentation declarations using the SDK's
locked json-schema-to-typescript16.0.0; no duplicate wire types were authored.
The exact SDK browser build was explicitly generated from both native exports,
then the client used only its public **type** exports. Fixture preparation checks
SDK source/artifact fingerprints, protocol and fixture hashes, producer schemas,
catalog binding and viewer binding before the build.

Focused verification on Node26.11.1:

- Fresh package-local `npm ci --workspaces=false --ignore-scripts`, then `check`:
  strict TypeScript, import/effect guards, 15 Node tests, fixture verification and
  production build passed. Both shared/root installs remained untouched.
- Ten production-browser cases passed, five each in Chromium158.0.8127.18 and
  Firefox159.0 via Playwright1.65.0. No failures/skips/retries. Cases cover real
  source text, same-name object identity, keyboard focus restoration, decision
  jump, 640px reduced-motion layout, route refresh, missing metadata, absent API
  requests and empty browser storage/service-worker state. Console/page errors
  are checked on the primary route under restrictive CSP.
- Desktop1280×720 and narrow640×720 screenshots were visually inspected, including
  a selected public commander. The own hand scrolls horizontally; smaller windows
  scroll vertically to retain all regions. This is an automated fixture inspection,
  not actual human play or a screen-reader/user usability study.
- Production module allowlist passed: only client source, the two exact public
  fixture assets and reviewed React/router runtime dependencies. No SDK runtime,
  Node built-ins, native internals, private fixture, strategy policy or executable IR
  enters the runtime bundle. Output: JS292.66kB / gzip92.41kB, CSS8.25kB / gzip2.40kB.
  These are build sizes, not latency/performance claims.

Retained local evidence: `generated/final-check.log`, `generated/browser-report.json`,
`generated/browser-results/`, `generated/fixture-verification.json`, and
`generated/bundle-inputs.json`. Native and SDK live-read proof is peer-produced
evidence; it was not rerun by this client lane. No native process was started here.

Decision coverage: generated inventory contains28 requests/33 responses. The
TypeScript switch is exhaustive over all28 requests; only `mulligan` renders a
reviewed read-only explanation. The other27 explicitly report unsupported, and
none submits a response. No M2 interaction coverage is implied. Multi-face active
mapping, compiler ability labels and action beginnings remain unavailable gates.

## Initial shell checkpoint

Pinned Node 26.11.1; React/React DOM 19.3.1, React Router 8.5.1, Vite 8.3.8,
TypeScript 6.0.3, Playwright 1.65.0. Package-local install with exact lockfile and
`--workspaces=false --ignore-scripts`; root node_modules was absent and untouched.

Implemented strict TS structure, import/effect guards, table and card/decision
inspectors, declarative routes, unknown-data fallbacks and explicit disconnected
Join. Pure helpers cover draft token/seat/match invalidation, candidate selection,
ordering, incarnation identity, exact quantity syntax, generation fencing and
lifecycle-only projection retention. Journal/store interfaces are specifications,
not implemented IndexedDB or a connected session controller.

Initial focused checks: nine Node tests, strict TypeScript, import/effect boundaries,
production build, six browser cases (three each Chromium/Firefox). Browser cases
exercise keyboard navigation, decision jump, route refresh, 640px reduced-motion
layout, no API requests and no browser persistence. Production fixture preview
uses CSP without unsafe-eval/unsafe-inline and with connect-src none. Screenshots
were inspected at desktop and narrow sizes. This is automated inspection evidence,
not an actual human play session, a performance measurement or SDK validation proof.

Meaningful early findings: TypeScript 7.0.2's package no longer exposes the legacy
AST compiler API; initial typecheck failed, so the compatible stable 6.0.3 line was
pinned. Exact-optional checking caught an optional CSS class prop mismatch; fixed
without weakening strictness. Source review added a final-newline quantity case
because JavaScript `$` alone does not enforce canonical decimal termination.
Vite emits React Router's `use client` directive warning for this client-only
bundle; production browser execution is checked. Playwright reports this Linux
distribution is not officially supported and uses its Ubuntu fallback binaries.

## Still gated after the fixture checkpoint

The native lane owns permitted observations and the offline source-text producer;
the SDK lane owns wire handling/generated types/browser proof. This client consumes
their pinned outputs and does not replace their contracts.

No live authentication, Web Locks, resume/stream integration, persistent journal,
submission, available-action query, privacy-paired real-host test, dense board or
natural game acceptance ran. No M0-wide/M1/M2/M3/M4 acceptance is claimed. Native
action beginnings, decision semantic context and compiler source-ability labels
remain later producer gates; the client will not substitute rule heuristics.
