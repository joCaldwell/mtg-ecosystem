# Controlled-creature damage replacement

The current unreleased schema remains **33**.
`ControlledCreatureDamageReplacementV1` adds the bounded static effect
`DoubleControllerCreatureDamageToPermanentOrPlayer`. The complete source witness
is **Gratuitous Violence**, Oracle ID `7c340a39-4ee0-4ba1-bb66-6674f8020fda`,
an Enchantment costing `{2}{R}{R}{R}`. Retained source SHA-256 is
`81f6aeae09379a1a2a140e3de8de31d667735641625eb646bea95825f11cda50`;
AST SHA-256 is
`1d2cfbf71360a37b4116a5b85129568efd976e11b0dbaada2f5020c2663e62a4`.
The frozen source, AST and nonexecutive diagnostic binding remain unchanged.

Strict lowering consumes exactly an individual controlled-creature damage
source, all damage kinds, permanent-or-player recipients and multiplication by
two for each occurrence. Partial filters, extra qualifiers, other multipliers
and narrower recipient/kind scopes reject the complete card. The global Furnace
effect retains its separate meaning and existing IR bytes. Both effects function
only on the battlefield; names and profile identities never select mechanics.

Applicability compares the replacement provider's current controller with the
damage source's current exact-incarnation properties, or its last-known
properties after departure. Static source-property replacements explicitly apply
to sources outside the battlefield under CR 609.7c. This differs from ordinary
controlled-creature damage triggers, which describe battlefield permanents.
Ownership, invocation controller and damage-source controller remain distinct.

Prepared damage retains immutable assignment facts. A separate bounded optional
source appearance retains the properties needed for replacement matching and
refreshes on departure or typed control changes. Control changes create these
facts even without a scoped replacement, so an unrelated permanent cannot enable
otherwise identical control-return behavior. No unbounded LKI archive is added.

Damage-time deathtouch, lifelink and its beneficiary use current source properties
or the latest departure appearance. Every prepared source's departure captures
that bounded LKI, including when no scoped replacement exists. Simultaneous
departures use the common prior battlefield. Original assignments and their
deathtouch lethal-assignment proof remain unchanged: losing deathtouch after a
legal one-damage trample assignment prevents its damage-time destruction result.
Consumed pre-event property/player facts use the same lookup as damage commit.

Each applied scoped replacement freezes its provider and damage-source
qualification separately. Later departure or control changes cannot undo that
consumed prefix. Restoration validates exact identities, immutable properties,
controller domains, multiplication and once-per-effect/member use; accepted-input
replay proves historical qualification. Candidate selection and application both
check current eligibility. Existing affected-recipient/APNAP ordering, prevention
budgets, zero-damage behavior, atomic commit and retention limits remain in force.

Public effect projections distinguish controlled-creature doubling from global
doubling without exposing trusted matching facts. The existing simulator damage
estimate conservatively omits the new scoped modifier. Historical catalogs and
profile digests stay unchanged; hosting/search admission does not expand.

Rules authority is the retained 2026-09-25 Comprehensive Rules, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`:
CR 104.3a, 109.5, 120.3f, 510.2, 608.2h, 609.7c,
611.3, 614.1/614.2/614.4/614.5/614.7a, 616.1, 702.2b/c/e and 702.15b/c.
Official source:
<https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>.

Focused validation passed formatting, dependency/staged-artifact boundaries,
workspace all-target compilation, 124 compiler tests, 24 contracts tests and 38
selected core tests. The core selection runs nine new replacement/property cases,
12 keyword cases, six existing effect cases, six attacking-keyword cases, four
ordinary damage-appearance cases and the real grouped-token lifetime regression.
All 39 executable source/schema/catalog checks and 47 SDK checks passed, including
strict generated TypeScript, 33 response variants and a generated public-effect
type/privacy proof. Trusted matching fields are absent from the public schema.
Smoke and explicit replay passed at build
`8804fe0e03df54a16a04abf87ea7b1883da559dd880909cbf000ee4c9385d568`,
with final state `b34e18dae91dbc123485bae4075754c0464e732ffb1296878d1d6a8a738751ac`.

The keyword correction first reproduced a real paid-equipment/ordering defect:
after Warhammer's owner conceded, its surviving pinger incorrectly retained
lifelink. Four departure-order controls now cover current properties, latest LKI,
positive LKI and common simultaneous LKI, with accepted-input replay. The old
Ohran blocker-destruction expectation was corrected following the cited rules;
the original legal 1/5 assignment and five player damage remain asserted.
No frozen card record or historical catalog/digest was rewritten.

Raw logs and failed attempts, smoke/replay, SDK manifest and public schema are in
`~/.local/share/mtg-ecosystem/engine-verification/correctness-loop-20261112/`,
under `controlled-replacement-` and `damage-keyword-timing-` prefixes. Full runtime
suites, full games, actual hosted clients, release/profile matrices and retention
rebuilds were omitted. This is focused validation, not combined acceptance or
expanded hosting/search review.
