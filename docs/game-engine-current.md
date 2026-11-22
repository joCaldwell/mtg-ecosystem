# Current engine map

The Rust engine executes finite Commander matches, reviewed search and local
hosting. Fixture/catalog admission defines support; the deck library is independent.
`packages/game-engine` is the default path root below.

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
[invariants](game-engine-mechanics.md#runtime-invariants) owns runtime constraints.
Later extensions retain separate qualification. Shared
rules/capacities never dispatch by card/profile name. Hosting admits foundations
and attachments; search admits foundations only. Later headless admissions do
not broaden those consumers. [Catalogs](game-engine-catalogs.md#ir-and-build-identity)
owns the sole active IR schema 1 and separate version/build identities.

Controller-creature, attacking-creature and controller-permanent
temporary keyword programs share bounded capture and characteristic derivation.
A combined fixed stat/keyword grant is one instruction/effect with one recipient
capture; later control or incarnation changes do not select new recipients.
Permanent-wide grants reject stat modifiers. Profiles and consumers remain
separately admitted.

The exact 100-card Ramos profile (`scripts/play-ramos.sh`) completed a seeded
four-seat headless game and replay; seat rotation, varied pods and hosted client
qualification remain open; see [evidence](game-engine-evidence.md#ramos-land-and-library-choice-integration).

Continuous Aura control shares one operation for creatures, artifacts, lands and
general permanents. Control-source dependencies use scratch, apply-once ordering;
attachment legality and timestamps remain shared. Multitype targets retain all
types. Structural attachment cycles and type-changing sources remain unsupported.

Product setup embeds a qualified sample; standalone client acceptance covers
joining and opening choices. Full decisions, automated launch, account-to-seat
provisioning and arbitrary library-deck admission remain unfinished. Product
sign-in grants no Rust seat; [client contracts](game-client.md) own integration.

## Evidence boundaries at the platform baseline

The platform baseline was `c62777b`. Historical acceptance covers D2c-c
`a7f5571` (ordinary cast triggers) and D2c-d `394301d` (declared-attack
triggers and reviewed demo/Bundle lifetimes). Later source-backed families retain
separate focused qualifications; acceptance never transfers to later source or
consumers.

[Evidence](game-engine-evidence.md) owns exact producer/source identities, checked
scopes, bridges, omissions and preserved proof locations. [Mechanics](game-engine-mechanics.md)
owns supported shapes; [catalog/profile contracts](game-engine-catalogs.md)
own admission.

Read active snapshot/public/query formats from source. Exact build/catalog
identity fences recovery and private artifacts; historical formats are not
compatibility promises. Refactor history remains in Git and indexed evidence.
