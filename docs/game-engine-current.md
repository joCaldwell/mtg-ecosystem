# Current engine map

The Rust engine executes finite Commander matches, reviewed sampled-world search
and local hosting with a generated SDK. Exact fixture/catalog admission, not
ordinary deck legality, defines support. The deck library is independent.
Paths in the authority table are relative to `packages/game-engine` unless named.

## Authorities and navigation

| Area | Authority / manual |
| --- | --- |
| Source parsing/archive | TypeScript `packages/oracle-parser`; [manual](../packages/oracle-parser/AGENTS.md) |
| Executable IR and complete-source lowering | Rust `crates/contracts` and `crates/compiler`; [catalog workflow](game-engine-catalogs.md) |
| Deterministic rules, decisions, visibility, replay | Rust `crates/core`; [mechanics and invariants](game-engine-mechanics.md) |
| Public-view policies and native sampled search | Rust `crates/sim`; [search contract](game-engine-search.md) |
| Admission pools | `tests/fixtures/catalogs` and `profiles`; [profiles](game-engine-catalogs.md#match-profile-data) |
| Hosting and generated protocol | `packages/game-server`; [host contract](game-engine-server.md), [SDK manual](../packages/game-server/sdk/AGENTS.md) |
| Environment, caches and execution | [Development/verification](game-engine-development.md), [storage](game-engine-storage.md) |

## Supported behavior and remaining work

[Mechanics](game-engine-mechanics.md) owns complete supported families/rejections;
[invariants](game-engine-mechanics.md#runtime-invariants) owns runtime constraints. New source-backed
keyword/control/compiler extensions retain separate qualification below. Shared
rules/capacities never dispatch by card/profile name. Hosting admits foundations
and attachments; search admits foundations only. Later headless admissions do
not broaden those consumers. [Catalogs](game-engine-catalogs.md#ir-and-build-identity)
owns the sole active IR schema 1 and separate version/build identities.

Ordinary controller-creature, attacking-creature and pure controller-permanent
temporary keyword programs share bounded capture and characteristic derivation.
A combined fixed stat/keyword grant is one instruction/effect with one recipient
capture; later control or incarnation changes do not select new recipients.
Permanent-wide grants reject stat modifiers. Profiles and consumers remain
separately admitted.
The Ramos basic/optional-cost cascade family has source-qualified evidence on
`2b83a482` (M3 exit). Combined `3813db05` qualifies complete Apex and Wanderer,
including independent self-cascades, live controller-creature haste, recovery and
strict cost/grant boundaries. Private/public-context-work are 26/21; prior producers
retain their own identities. No exact-target, profile, host, SDK or search admission
follows. The permanent-set and typed Aura proofs remain separate; their current Ramos
combination is unqualified.

Continuous Aura control shares one typed operation for creatures, artifacts and
lands. Attachment legality, semantic timestamps and controller reconciliation are
shared; artifact creatures and artifact lands retain all their types. General
permanent-control dependencies and type-changing sources remain unsupported.

Product setup embeds a qualified sample; standalone client acceptance covers
joining and opening choices. Full decisions, automated launch, account-to-seat
provisioning and arbitrary library-deck admission remain unfinished. Product
sign-in grants no Rust seat; [client contracts](game-client.md) own integration.

## Evidence boundaries at the platform baseline

The platform baseline was `c62777b`; acceptance never transfers to later source:

| Boundary | Status |
| --- | --- |
| D2c-c `a7f5571` | Accepted ordinary cast triggers at historical IR23 |
| D2c-d `394301d` | Accepted declared-attack triggers at historical IR24, including reviewed demo/Bundle lifetimes |
| Later D2c-e/IR25 and finite trigger/keyword/interaction additions | Candidate/focused source qualification; no renewed combined acceptance or P7 performance claim |
| Schema-1 reset | Regenerated 29 active pools, 1,357 definitions unchanged; focused checks only |
| Ramos combined M4a/M4b | Source `3813db057d66bd8d9ce3de7825a6758a2f82dd30`; 542 distinct Rust, 2,748 schema probes, 944 source rows; focused qualification only |
| Typed Aura-control feature | Source `97afa6cdcdda88d76a86459b18ae53cda63c1546`; 837 compiled rows; 507 distinct Rust tests (398 precheck passes through a private-test-only bridge, 109 fresh), 3,021 schema probes; 26/21 unchanged; feature-only pending client source hold |
| Controlled-permanent keyword feature | Source `21d10aaa67ea67f009366cbce0d754b59d9b7daa`; 831 compiled rows; 90 fresh distinct core tests plus 375 source-bridged compiler/contracts passes, 2,805 schema probes; 26/21 unchanged; feature-only pending client main-publication hold |
| Prior atomic targeted-keyword repair | Source `8462ce4c362702da5062762e45be00f4693080d9`; 827 compiled rows; focused rules/policy/pod/recovery/schema qualification, snapshot 26 and public/context/work 21; consumers unqualified |
| Prior captured creature-set keyword family | Source `0eabb031831f041eed3ce1506be8bc516718038a`; 825 compiled rows; focused rules/recovery/schema qualification, snapshot 25 and public/context/work 20; consumers unqualified |
| Prior temporary combat-keyword family | Source `b7d5f51f02b4cd523ab656bc979b04b4939a9643`; 814 compiled rows; focused compiler/contracts/rules/recovery/schema qualification only |
| Latest compiler input rejection | Source `0eb054cfb50cc7b0d061392d53825017154c8d0e`; compiler-only malformed-filter qualification, separate from runtime producers |
| Client/hosting consumers | Retain their own exact producer/schema/catalog/build qualification, separate from current engine HEAD |

[The evidence guide](game-engine-evidence.md) retains exact source/digest and proof
locations, historical failures and omissions. At the platform baseline trusted
snapshot/bundle formats were 19/20, public projection/context/sidecar 16/16/15.
The architecture refactor preserved snapshots/public formats and advanced query
work sidecars to 16 with one charged initialization unit; old sidecars reject.
Read the source constants before changing any current format.
