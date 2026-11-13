# Tapped-permanent target filters

The unreleased Card IR schema remains **1**. `PermanentFilter.tapped: true`
requires the current battlefield incarnation to be tapped. An absent or false
field imposes no status restriction; it does not mean “untapped.” Existing
type, exclusion, controller, targetability and exact-incarnation restrictions
still intersect with this predicate.

The compiler consumes the complete source filter and accepts only the singleton
`status: ["tapped"]`. Empty, null, multiple, unknown and other status predicates
reject. Other source qualifiers must still fit the existing filter contract.
This uses the existing permanent-removal instructions, not a card-name dispatch.

Contracts infer `TappedPermanentTargetV1` from every program's filters, including
modal children, activations and triggers. Stack-spell filters reject tapped
status: only battlefield permanents have status. Hosting and sampled-search
capability allowlists have not admitted this new predicate.

Object-target programs must share both domain and filter across their slots.
The current request exposes a common candidate set, and runtime checks use that
common predicate. Different per-slot filters reject explicitly until the request
and execution contracts support them. Equal-filter multiple occurrences remain
valid; this guard also applies independently of tapped status.

## Frozen source

Both fixtures were extracted through the generic catalog extractor from the
accepted full archive with SHA-256
`d705222b280a71cb5a5362f657d530b22402ca51687bac3227c1d6db403d0282`.
Their complete Oracle text is “Destroy target tapped creature.”

| Fixture / Oracle ID | Source facts | Physical archive line |
| --- | --- | --- |
| `5820d760-8ed7-4abf-a959-c06ed86a3730.json` (Swift Response) | Instant, `{1}{W}`, white identity, Commander legal | 13231 |
| `ce979f36-84f2-4419-b400-971263494cc6.json` (Assassinate) | Sorcery, `{2}{B}`, black identity, Commander legal | 31148 |

Both source ASTs bind exactly one tapped creature target. The fixture bytes,
source/AST/binding hashes, accepted archive pins and original source indices are
retained in `tests/fixtures/` provenance. Extraction verifies the full archive
before writing; the separate verifier checks exact source lines afterward.
Neither fixture changes catalog or profile membership.

The rules basis is the pinned [2026-09-25 Comprehensive Rules](https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt):
110.5 defines status, 601.2c requires appropriate targets, 608.2b rechecks targets
at resolution, and 701.8a–b defines destruction. Candidate generation,
announcement validation and resolution share core's predicate. Develop policy
uses only the existing public `VisibleObject.tapped` value.

## Focused verification scope

Source tests cover both complete records, strict status rejection and complete
filter consumption. Contract probes cover capability requirements, nested
programs, invalid stack status and rejection of unequal slot filters. Runtime
witnesses exercise actual combat and mana taps, paid destruction, invalid target
atomicity, query purity, timing and replay across budgets and trace modes.
An explicitly synthetic untap probe covers a status change during payment;
it does not claim a source-backed untap spell or natural replay witness.

The focused pass executed 28 contracts, 138 compiler, five new core, 34 adjacent
removal, eight blocking-target and 21 policy tests: 234 passed. Workspace
all-target compilation, formatting, structure checks and production
compiler/core/simulator Clippy passed. Contracts Clippy retained an unchanged
`collapsible_if` finding in grouped-damage validation; it was not suppressed or
reported as passing. Raw receipts, failed attempts and source fingerprints
are retained under `generated/tapped-targets/` in the development checkout.

Existing pool outputs remain byte-qualified by their own source and catalog
identity. This extension does not renew full-profile acceptance or any client
producer qualification; no client SDK/browser or release/profile matrix result
follows from these focused witnesses.
