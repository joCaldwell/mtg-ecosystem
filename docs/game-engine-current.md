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
Ramos source `802aab64` combines main `a39dc880`, counted affinity and once-locked
casting totals with prior cascade/history/counter/control families. Qualified665
distinct Rust/4,696 schema/978 rows use private30/public-context-work23,IR1;
unchanged-library bridges retain original producers. The current85-source audit is
24 lowered/validated,61 Unsupported,zero Invalid. Exact-target/profile/host/SDK/search
admission remains open; historical client acceptance stays separate.

Continuous Aura control shares one operation for creatures, artifacts, lands and
general permanents. Control-source dependencies use scratch, apply-once ordering;
attachment legality and timestamps remain shared. Multitype targets retain all
types. Structural attachment cycles and type-changing sources remain unsupported.

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
| Ramos combined M4e | Source `802aab64`;665 distinct Rust/4,696 probes/978 rows;30/23; exact producers/bridges in evidence; focused qualification only |
| Independent typed Aura main | Source `f3326dcd`; 605 distinct Rust, 3,021 schema, 841 rows; 26/21; separate from Imoti and consumer acceptance |
| Controller Scry | Focused headless qualification; [proof and exclusions](game-engine-evidence.md); consumers remain separate |
| Controlled-permanent scalar keywords | Rules `7315ffe2`, schema `44930622`; 626 distinct Rust/633 executions, 4,316 probes, 848 rows; explicit bridges; IR1/29/23; consumers unqualified |
| General Aura-control dependencies | Source `aa28e9b7a39d601a8a9a8bb4fbde9484fe20fc1f`; 843 rows; 550 distinct Rust tests, 3,190 probes; explicit private-test source bridge, IR1/26/21; consumers unqualified |
| Client/hosting consumers | Retain their own exact producer/schema/catalog/build qualification, separate from current engine HEAD |

[The evidence guide](game-engine-evidence.md) retains exact source/digest and proof
locations, historical failures and omissions. At the platform baseline trusted
snapshot/bundle formats were 19/20, public projection/context/sidecar 16/16/15.
The architecture refactor preserved snapshots/public formats and advanced query
work sidecars to 16 with one charged initialization unit; old sidecars reject.
Read the source constants before changing any current format.
