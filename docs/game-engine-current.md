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
| Permitted wire values, tensor encoding, batched Python simulation | Rust `crates/view`, `crates/observe`, `crates/env`; [Python interface](game-engine-development.md#batched-numpy-environment) |
| Match play, public-view policies, pod benchmarking, analytics and native search | Rust `mtg_sim::{play, policy, pods, analytics, search, mana_spend}`; [search contract](game-engine-search.md) |
| Admission pools | `tests/fixtures/catalogs` and `profiles`; [profiles](game-engine-catalogs.md#match-profile-data) |
| Hosting and generated protocol | `packages/game-server`; [host contract](game-engine-server.md), [SDK manual](../packages/game-server/sdk/AGENTS.md) |
| Environment, caches and execution | [Development/verification](game-engine-development.md), [storage](game-engine-storage.md) |

The simulator binary installs its build identity and owns live-journal files and
wall-clock reporting. Library callers can supply their own journal sink to
`play::run`. The [verification topology](game-engine-development.md#engine-test-topology)
maps the four integration suites and eight library harnesses.

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

## History

Read active snapshot/public/query formats from source. Exact build/catalog
identity fences recovery and private artifacts; historical formats are not
compatibility promises. Historical acceptance, counts and timings bind their own
source revisions and never transfer to HEAD; [evidence](game-engine-evidence.md)
keeps their immutable locators. Refactor history remains in Git.
