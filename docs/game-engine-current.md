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

Archive R4 composition feeds the sole executable instruction,
`Instruction::Kernel`. The KIR checker admits complete programs with bound
operands. Core resolves them through its rules transactions and continuation
stack. The compiler reports an unsupported record when a clause cannot lower.
[Mechanics](game-engine-mechanics.md) describes the rules and [catalogs](game-engine-catalogs.md#kernel-ir)
describes the wire contract and admission boundaries.

The retained Commander Foundations profile uses the R4 match catalog. Its
decklists, catalog digest and engine build bind the host and search inputs.
The smoke and rules witnesses use smaller source-lowered catalogs. Historical
card counts, deck completions and benchmark timings belong to the source
revisions in [evidence](game-engine-evidence.md).

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
