# 🤖 AI Agent Onboarding Manual

This is the **root** manual: what the whole project is, and how to find the
sub-project you've been asked to work on. Each package carries its own
`AGENTS.md` with the commands, development loop, and constraints that apply
inside it. Those are authoritative for their package; this file is not.

---

## ⛳ First: establish which project this session is about

The packages here are independent, at different stages, with **different test
runners, different dependency rules, and different constraints**. Applying one
package's conventions inside another produces confidently wrong work.

**If the request does not make the target obvious, ask before doing anything.**
A file path, a package name, or an unambiguous subject ("the deck's brief",
"the parser grammar") all count as obvious — this is not a reason to interrogate
someone who has already told you where they are. It's a reason not to guess
when they haven't.

Once identified, **read that package's `AGENTS.md` before touching its code**,
and follow it over anything general you might assume.

Josh may also open a package directory directly in the editor, in which case
that package's `AGENTS.md` is the one in scope and no routing question is
needed.

| Package | Layer | Status | Manual |
| --- | --- | --- | --- |
| **[deck-builder](packages/deck-builder/AGENTS.md)** | 3 (built early) | ⭐ **Active** | [AGENTS.md](packages/deck-builder/AGENTS.md) |
| [oracle-parser](packages/oracle-parser/AGENTS.md) | 0 | ⏸️ Paused | [AGENTS.md](packages/oracle-parser/AGENTS.md) |
| [card-data](packages/card-data) | 0 | Output artifact — compiled Card IR, not hand-edited | — |
| [game-engine](packages/game-engine) | 1 | Verified finite Commander profile and sampled search | [AGENTS.md](packages/game-engine/AGENTS.md) |
| [game-server](packages/game-server) | 2 | Verified finite local host | [AGENTS.md](packages/game-server/AGENTS.md) |

The finite P8 release has verified 526 Rust debug tests plus 40 Node checks,
526 release tests, and 40 SDK tests with strict TypeScript. Those three passed
phases were revalidated after the user-requested pause; the interrupted retention
phase was not upgraded to a pass. Current-build search also passed all sixty
primary jobs and independent replay: p95 2,662,119µs, maximum 2,690,433µs,
394,698,752-byte whole-process peak, and 4,188 useful trials with 3,012 unstarted
trials excluded. Offline reconstruction and independent native/server audits
also passed; all private proof bytes are retained outside this worktree. The
original phase logs remain unchanged across the pause and two host-tool fixes.
See the package manuals for the exact build and evidence boundaries.

---

The D2c-a cards-as-data increment adds generic beginning-step triggers at IR v21,
with typed replayable event witnesses and trusted snapshot/bundle versions 19/20.
It remains outside hosting/search reviewed capability sets. Verification passed
959 Rust tests (18 existing diagnostics ignored), 57 Node checks, 114 focused
release checks, 47 SDK tests/strict TypeScript, and all 216 unchanged profile games
and explicit replays. Compiler coverage is 1,923/32,116 legal records (5.98767%),
nineteen additions and zero losses from D2b. See
[the D2c-a report](docs/game-engine-cards-as-data-d2c-a.md) for exact scope,
representation proof, source/executable identities and preserved failures.
Independent parent review and the separate retained-evidence/representation audit
passed; no new quiet P7 claim is made.

---

D2c-b adds frozen event-player recipients for mandatory draw/life programs and
source-status intervening conditions at IR v22. Status reads the exact source's
current battlefield information or departure-refreshed LKI, independent of traces
and bounded by existing live continuations. Snapshot/bundle remain 19/20 and public
versions 16/16/15; all old catalog bytes and 173 historical state hashes are unchanged.
Verification passed **976 Rust tests (18 existing diagnostics ignored), 58 Node,
131 focused release checks, 47 SDK tests/strict TypeScript**, and all **216 games
and 216 explicit replays** across twelve unchanged profiles. Coverage is
**1,929/32,116 (6.00635%)**, six additions and zero losses from D2c-a. New
capabilities stay outside reviewed hosting/search sets; Master of the Feast's
multiplayer-draw rejection and runtime guard remain. Parent independent review is
pending. See [the D2c-b report](docs/game-engine-cards-as-data-d2c-b.md) for source witnesses, identities and retained evidence.

---

## 🌌 The project: Project Multiverse

An initiative to digitize Magic: The Gathering into a modular, layered,
**agent-first** system, built bottom-up. Three principles drive it:

1.  **The game is the API** — every layer exposes clean, documented interfaces.
2.  **AI is a first-class citizen** — agents consume card data, query rules,
    and play games natively, rather than through a UI meant for humans.
3.  **Bottom-up construction** — each layer is tested and documented before the
    next begins.

```
Layer 3  📱 Clients & agent apps      ← deck-builder lives here
Layer 2  🌐 Game server & API
Layer 1  ⚙️  Game state & rules engine
Layer 0  🔮 Oracle text parser & Card IR
```

See [docs/architecture.md](docs/architecture.md) for the full layer breakdown
and [README.md](README.md) for the project overview.

### Why the active project is at the top of the stack

The build order above is the plan, and Layer 0 is genuinely the foundation —
but the parser's remaining work is its hard tail (the rules layer system,
replacement effects, complex targeting), where a wrong AST is worse than no
AST. That work is **paused pending a stronger model**.

The deck-builder was pulled forward instead, and it is deliberately
**standalone**: it ingests Scryfall bulk data into its own SQLite database and
does not consume the Card IR or the rules engine. That's what makes it
buildable now. Two consequences worth knowing:

*   It cannot answer questions that need real rules evaluation. Where the spec
    wants that — companion deck-building conditions, for instance — it says so
    and asks the user to verify by hand, rather than guessing.
*   When Layers 0–1 are finished, the deck-builder is the natural first
    consumer. Nothing in it should make that harder, but nothing in it should
    wait for it either.

---

## 📚 Shared documentation

Project-wide docs in [docs/](docs/), relevant regardless of package:

| Doc | Contents |
| --- | --- |
| [architecture.md](docs/architecture.md) | Layer breakdown and build order |
| [decisions.md](docs/decisions.md) | Architectural decision log, with rationale |
| [glossary.md](docs/glossary.md) | MTG and system terminology |
| [project-structure.md](docs/project-structure.md) | Directories and workspaces |
| [scryfall-integration.md](docs/scryfall-integration.md) | Bulk data ingestion |
| [agent_design.md](docs/agent_design.md) | Agent-first integration philosophy |
| [deck-builder-spec.md](docs/deck-builder-spec.md) | Full spec for the active project |
| [oracle_parser.md](docs/oracle_parser.md) | Parser design, AST, IR format |
| [data_schemas.md](docs/data_schemas.md) | Input and output JSON schemas |

---

## 🧭 Conventions that hold everywhere

*   **Adopted stack: TypeScript + Rust.** TypeScript owns ingestion, parsing,
    AST archives, the deck-builder, browser applications, and the client SDK.
    Rust owns the semantic compiler, rules engine, native simulation/search
    and agent policies, and game server. Browser-local rules use the Rust core
    compiled to WebAssembly. Language selection is complete; follow the
    [language decision](docs/runtime-language-decision.md).
*   **npm and Cargo workspaces.** `npm install` at the root links the Node
    workspaces. Cargo builds the initial compiler/engine/scenario crates;
    `npm run check:engine` runs their checks. Root scripts drive the parser
    pipeline; the deck-builder has its own scripts. Use package-specific checks.
*   **Card facts come from card data, never from model memory.** Card names,
    oracle text, and legality are all checkable — check them. This is the
    project's founding premise, not a per-package preference.
*   **Verify external formats against their own documentation.** Scryfall
    syntax, Archidekt's import format, the Comprehensive Rules: read the
    source and cite it in a comment. These are exactly the details that get
    confabulated.
*   **Prefer clean design over backward compatibility.** Everything here is
    early-stage and single-user. Rewrite rather than shim.
*   **Don't commit generated or downloaded artifacts.** `generated/`,
    `.scryfall-cache/`, `dist/`, and the deck-builder's `data/` and `.dev/`
    are all ignored.


D2c-c ordinary cast triggers are a **source-reviewed implementation candidate;
final combined acceptance remains pending**. IR23 separates caster
relations from compositional spell-type predicates, collects only after successful
complete casting/payment, and retains frozen typed cast witnesses. Snapshot/bundle
stay 19/20; old catalog/history references are unchanged. Hosting/search exclude
the new capability. Implementer checks are focused only; the parent owns final
acceptance. See `docs/game-engine-cards-as-data-d2c-c.md` for exact scope, source
witnesses, coverage and retained development evidence.

D2c-d ordinary **per-creature declared-attack** triggers are a separate
implementation candidate for independent source review and future acceptance.
IR24 adds exact-source/compositional attacker predicates and frozen typed
attacker/defender witnesses using the real declaration batch occurrence. Existing
snapshot/bundle/public versions and old catalog/history references are unchanged;
hosting/search exclude the capability. Implementers run focused checks only.
See `docs/game-engine-cards-as-data-d2c-d.md` for exact source, scope and evidence.
