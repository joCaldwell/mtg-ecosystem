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
"the ANTLR grammar") all count as obvious — this is not a reason to interrogate
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
| [game-engine](packages/game-engine) | 1 | 🕳️ Not started (README only) | — |
| [game-server](packages/game-server) | 2 | 🕳️ Not started (README only) | — |

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

*   **TypeScript monorepo, npm workspaces.** `npm install` at the root links
    packages. Root scripts drive the parser pipeline; the deck-builder has its
    own scripts in its own package.json.
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
