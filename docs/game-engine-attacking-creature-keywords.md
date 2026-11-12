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
Existing pre-damage source facts and simultaneous-departure LKI retain the keyword
when appropriate. Losing a grant before damage preparation changes the current
lethal trample assignment requirement; a prepared actual source retains its
frozen damage facts.

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
