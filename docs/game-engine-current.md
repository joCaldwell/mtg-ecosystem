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
wall-clock reporting; library callers can supply their own journal sink to
`play::run`.

## Supported behavior and remaining work

Archive R5 composition lowers each supported card to `Instruction::Kernel`
programs or reports an unsupported record; [mechanics](game-engine-mechanics.md)
owns the supported rules and [Kernel IR](game-engine-catalogs.md#kernel-ir) the
wire contract and admission boundaries. The retained Commander Foundations
profile uses the R5 match catalog; its decklists, catalog digest and engine build
bind the host and search inputs. Smoke and rules witnesses use smaller
source-lowered catalogs.

Product setup embeds a qualified sample; standalone client acceptance covers
joining and opening choices. Full decisions, automated launch, account-to-seat
provisioning and arbitrary library-deck admission remain unfinished. Product
sign-in grants no Rust seat; [client contracts](game-client.md) own integration.

Read active snapshot/public/query formats from source. Exact build/catalog
identity fences recovery and private artifacts; historical formats are not
compatibility promises. Historical counts, acceptance and timings bind their own
source revisions in [evidence](game-engine-evidence.md) and never transfer to HEAD.
