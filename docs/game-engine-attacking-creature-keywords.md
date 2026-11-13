# Attacking-creature deathtouch and retained Snow

IR33 adds `AttackingCreatureKeywordsV1`, the finite
`CreatureScope::AttackingController` scope, and `SnowSupertypeV1` with an optional
`Characteristics.snow` flag. The new static scope grants exactly deathtouch with
zero power/toughness modification and no restrictions. Other keyword grants,
mixed modifiers, status filters and scope qualifiers reject. Existing equipment,
aura, controller anthem and source-restriction scopes retain their contracts.

The complete source witness is **Ohran Frostfang**, Oracle ID
`b99ada26-9a61-4175-9fb8-15a106960220`, retained unchanged in `cards-data-d2ce/`.
It is a Snow Creature — Snake, 2/6, costing `{3}{G}{G}`. Block 0 grants deathtouch
to attacking creatures its controller controls. Block 1 uses the existing IR30
controlled-creature combat/player trigger and draws one card for each occurrence.
Source SHA-256 is
`ad3c05e73a2bea9195e1c0560520f900e3cf89a6729c8b290a49d8465f384c3d`;
AST SHA-256 is
`0d074eeb13886e07c9b88311e102fd9e42ec3d32e3261c86aefff1e6560851ce`.

The scope matches current battlefield controllers and exact attacking object
incarnations. The grant source need not attack. Tapping, attack readiness, active
player and whether a defender remains alive do not define attacking status.
Recipients are evaluated dynamically; granted keywords are a deduplicated union.
Assignment facts and simultaneous-departure LKI retain their separate meanings.
Losing a grant before preparation changes lethal trample assignment requirements.
Losing it after preparation preserves assigned quantities, while actual damage
checks the surviving source's current keyword or its latest departure LKI.
The later [damage-source appearance correction](game-engine-controlled-damage-replacement.md)
fixes the former frozen-keyword expectation without rewriting assignments.

Snow is retained on non-mana creatures only. Snow lands, other Snow card kinds,
Snow mana producers, `{S}` payments and Snow selectors remain unsupported. The
current color/quantity mana pools cannot retain Snow source provenance; the
adapter must not discard that property. False flags are omitted, so every prior
nonsnow catalog and payload keeps its bytes. Capability and actual semantic
field/scope guards require IR33 independently of labels.

The Snow flag is an additive optional, nonnullable public schema field when
characteristics are visible. Rust DTOs remain authoritative; generated SDK
artifacts are ignored, and strict generated-type checks cover both catalog and
observation characteristics. Existing snapshot/public format numbers remain
unchanged, with exact engine build identity still fencing recovery. New
capabilities remain outside hosting/search review. No historical catalog or
profile changes; the v2 headless pool remains IR32 with its original digest.

Rules authority is the retained 2026-09-25 Comprehensive Rules, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`:
CR 205.4g defines Snow; 107.4h distinguishes Snow provenance from mana color/type;
506.4/508.1k define attacking membership; 611.3a makes the static recipient set
dynamic; 613.1f governs keyword grants; 702.2b–f/702.19b govern deathtouch,
redundancy, LKI and lethal trample assignments. Official source:
<https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>.

Focused validation at source checkpoint `1854e37` passed formatting, dependency
boundaries, workspace all-target compilation, 118 compiler tests, 22 contracts
tests and 33 selected core tests. The runtime selection executes all six new
Ohran cases, 16 attachment cases, six filtered-damage cases, one token-combat
case and four v2 admission cases. The ordinary SDK schema binary was rebuilt;
all 37 executable engine source/schema/catalog cases and all 47 SDK checks passed,
including strict generated TypeScript and the unchanged 33 response variants.
The optional Snow field is proven on both generated catalog and visible
characteristic types without a hand-maintained DTO.

The first native run retained a stale IR30 whole-Ohran unsupported assertion.
Source/rules review confirmed that IR33 deliberately completes both previously
unsupported dimensions. The test now checks the complete new source and explicit
IR32 rejection with stripped capability labels. No source record, historical IR
fixture, runtime guard or expected card result was changed to bless a failure.

Raw logs, failed and passing native runs, public schema and SDK manifests are
retained in
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261112/`:
`attacking-keywords-focused-01.log`, `attacking-keywords-focused-02.log`,
`attacking-keywords-schema-01.log`, `attacking-keywords-sdk-01.log` and the
`attacking-keywords-sdk-verification-01.*` / `attacking-keywords-public-schema-01.json`
artifacts. Full runtime suites, actual hosted clients/recovery, a new full game,
release/profile gameplay matrices and retention rebuilds were not run. The
previous v2 match evidence keeps its original source qualification; these results
do not renew combined acceptance or host/search admission.
