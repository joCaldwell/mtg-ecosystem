# System Architecture

This document describes the full layered architecture of the MTG Ecosystem. The intended dependency order is **bottom-up**. The standalone deck-builder was pulled forward and is the active package; it does not depend on the parser or rules engine. The parser remains a limited supported subset, with broader work paused.

The language stack is **adopted**. An initial compiler/runtime scenario exists;
full games, search, and server implementation remain future work. Start with the
[implementation plan](game-engine-plan.md), then the
[engine specification](game-engine-spec.md) and
[runtime language decision](runtime-language-decision.md). Commander is the
only planned game format, with four-player headless simulation and agent search
as the primary workload. TypeScript owns ingestion, parsing, the deck-builder,
browser applications, and client SDK. Rust owns semantic compilation, rules
execution, simulation/search/native agents, and the game server. Browser-local
rules reuse the Rust core compiled to WebAssembly.

---

## 🗺️ Build Order

```mermaid
graph LR
    classDef done fill:#065f46,stroke:#10b981,color:#fff;
    classDef active fill:#1e3a5f,stroke:#3b82f6,stroke-width:3px,color:#fff;
    classDef future fill:#1f2937,stroke:#475569,color:#6b7280;

    M1["Milestone 1<br/>Oracle Parser<br/>& Card IR (limited subset)"]:::future
    M2["Milestone 2<br/>Game State<br/>& Rules Engine"]:::future
    M3["Milestone 3<br/>Game Server<br/>& API"]:::future
    M4["Milestone 4<br/>Clients &<br/>Agent Apps (deck-builder active)"]:::active

    M1 --> M2 --> M3 --> M4
```

---

## 💾 Layer 0: Oracle Text Parser & Card IR

**Goal**: Parse every MTG card's oracle text into a typed AST, then compile it into per-set JSON IR files that downstream layers consume.

**Key components**:
- **Ingestion**: Pull raw card data from Scryfall bulk data.
- **Normalizer and lexer**: Preserve ability boundaries and tokenize text without dropping unknown characters.
- **Hand-written parser**: Builds typed AST nodes directly from supported productions.
- **AST contract**: `packages/oracle-parser/src/ast.ts`; explicit logic, event grouping, zones, restrictions, and scope.
- **Verification**: Structural and negative tests, cursor invariants, and reproducible corpus acceptance reports.
- **Semantic validation and IR emitter** (future, Rust): Resolve references, check consumer capabilities, and emit versioned Card IR under `packages/card-data/`. The compiler crate lives with the engine to share executable contracts.

**Stack**: Ingestion/parser/archive use zero-runtime-dependency TypeScript,
executed directly by Node ≥ 23. The semantic compiler uses Rust and exchanges
versioned JSON artifacts with the parser and engine.
`npm run build-ir` fails explicitly. The Rust compiler lowers the frozen first
scenario subset; parsed ASTs themselves are never executable Card IR.

See **[Oracle Parser Design](oracle_parser.md)** for full details.

---

## ⚙️ Layer 1: Commander Game State & Rules Engine

**Goal**: A deterministic state machine that takes a `GameState` + `Action` → produces a new `GameState` + `Events`. Consumes the compiled Card IR from Layer 0.

**Stack**: Rust for the engine, native simulation/search harness, and in-process
agent policies. One core also supplies the browser WebAssembly rules module.

The proposed design also makes pending player decisions, serializable execution
continuations, private observations, snapshots/forks, and resource suspension
explicit. Internal mutation is an implementation choice; this conceptual
transition does not require a full-state clone for every action.

**Key subsystems** (future):
1. **Turn & Phase State Machine**: Track phases, steps, priority passes, and turn order.
2. **Stack & Resolution**: LIFO stack for spells and abilities, including targeting and resolution.
3. **MTG Layer System (Rule 613)**: Evaluate continuous effects in the correct dependency order.
4. **State-Based Actions**: Check-loop for lethal damage, legend rule, 0 toughness, etc.
5. **Triggered Ability Controller**: Detect and order triggers (APNAP).
6. **Dynamic Rule Registry**: Track replacement effects, restrictions, and modifications.

The first delivery is complete four-player headless Commander pods for a small
audited card profile, with deterministic replay and branchable state. Commander
setup/deck rules, command-zone movement, tax, commander damage, multiplayer
priority, and player departure are early requirements. Semantic compilation and the
engine are developed together on that subset; 98% syntax acceptance is not a
prerequisite for beginning Layer 1. See the
[milestone gates](game-engine-plan.md#3-milestones-and-dependencies) and
[Card IR boundary](game-engine-spec.md#3-card-compiler-and-executable-contract).

---

## 🌐 Layer 2: Game Server & API

**Goal**: Expose the rules engine over a network API for Commander pods, replays, and agent integration.

**Stack**: Rust server with direct core calls. Network/session orchestration
and remote-agent coordination stay outside the deterministic rules core.

**Key components** (future):
- Real-time event streams (WebSocket or gRPC).
- Semantic action space API — clients query legal actions, not guess.
- State diffs for efficient syncing.
- Explanation API for rules tracing.

---

## 📱 Layer 3: Ecosystem Applications

**Goal**: Client-side software built on top of the API layer.

**Stack**: TypeScript browser applications and client SDK, with HTML/CSS for
presentation. The standalone deck-builder retains its TypeScript backend,
frontend, and MCP tools. Native simulation agents run in Rust; external agents
connect through the server protocol.

**Applications**:
- **Game Client**: Board visualization, player input, animations.
- **Deck Builder** (active, standalone): Uses its own Scryfall SQLite database. Future integration must not delay its current development.
- **Draft Simulator**: Booster drafting against AI or humans.
- **AI Agents**: Play assistants, rules advisors, and playtesting bots.
