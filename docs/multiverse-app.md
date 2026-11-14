# Unified Multiverse application

Owner direction, 2026-11-14: deck building and playing belong in one product,
with one account and a shared deck library. The first game journey is one human
against three automated opponents. Public discovery and social features follow
later. The owner explicitly permits substantial internal refactors to keep the
integration clean; the solo playtester is not an accepted design reference.

## Responsibilities

| Component | Owns |
| --- | --- |
| `multiverse` application | Browser application, account/session UX, product navigation, Home, deck editor, game setup and process entrypoint |
| `deck-builder` backend | Existing account/auth implementation, Actor authorization, deck capabilities, SQLite data and MCP, behind public host/contracts entrypoints |
| `game-ui` | Router-free, effect-free display components over permitted display models; scoped theme; no account, protocol, fixture or rules authority |
| `game-client` | Generated-protocol adapters, qualified fixture preview, local decision drafts, live session/ownership/journal recovery, standalone integration harness |
| Future game coordinator | Account-to-match/seat association, immutable deck snapshot, admission, provisioning and automated-opponent supervision |
| Rust host/core | Authoritative supported-catalog admission, rules, legality, visibility, durable commands and outcomes |

The application consumes explicit package entrypoints, not private source paths.
The two actual table consumers use the same display implementation. Game styles
are scoped to their surface and React is supplied by the host. Neither an iframe
nor a second sign-in/session provider is an integration boundary.

The shared display model is a local view model, not an alternative gameplay wire
contract. Producer projections and generated SDK contracts remain authoritative.
No compiler, executable IR, trusted snapshot or bot credential enters the bundle.
The deck library retains its independent source database and release behavior.

## Top-level ownership

Account and product navigation belong in `packages/multiverse`, rather than the
deck-builder feature. The complete browser application now lives there: moving
only its shell would create cycles with the deck UI's existing providers, API
client and generic controls. Deck editing remains a named feature within the product.
Use a top-level application as the owner rather than a speculative navigation
library with one consumer.

The product consumes only the backend's public host factory and type-only
contracts. Its process owns the frontend build and host composition. Existing
backend authentication and account lifecycle remain one implementation alongside
their Actor/SQLite capability layer; this is not a second account system or an
auth/database migration. Extracting that backend identity foundation later must
first separate deck-specific authorization from principal/transaction primitives.
Preserve all current storage defaults, environment settings, credentials, MCP and
public URL formats when changing the process/build owner.

## Page scaffold

- `/`: Home, with a Start a game entry and actual account-scoped deck information.
- `/decks`: the existing library; existing deck, review, public and token URLs
  remain available. Home does not replace the library's capabilities.
- `/play`: setup for the first mode, a human and three automated opponents.
  An optional owned deck selection carries its real revision and commander data.
- `/play/preview`: the qualified opening sample inside a compact product frame.
  Optional selected-deck context controls return navigation; it never relabels
  the sample as that deck or claims a real match exists.

The shared shell covers account, library and setup. The table uses compact chrome
so the board retains space. Phone navigation remains usable; game-specific
desktop constraints are explicit. Route construction belongs in the existing
typed `href()` authority and new paths belong in the server's SPA allowlist.

There is no fabricated active-game list or result screen. The current library
summary has creation time, not last-viewed time, so a deck section must not claim
recent activity from IDs or revision numbers. An unreadable selected deck stays
unavailable; it must not silently select another deck.

## First increment and launch boundary

The first increment connects the real account, library and setup pages and embeds
a local source-qualified table preview. It does not implement authenticated match
creation or claim arbitrary builder decks can run. The current native host is
operator-provisioned, and current browser acceptance covers opening decisions.
Real Start game stays unavailable with an explicit explanation; Preview table is
a separate action that sends no gameplay commands and persists no game record.

Before real launch, implement the game coordinator as an application service:

1. Resolve the existing account to an Actor and recheck ownership below HTTP.
2. Read the exact selected revision and capture Oracle IDs, roles and quantities,
   excluding maybes and unrelated notes. Preserve this immutable game deck even
   when the builder deck changes later. Revalidate revision atomically at launch.
3. Request native admission against exact source/catalog/build/profile identities.
   Ordinary Commander legality is distinct from supported engine semantics.
   Multiple commanders and companions require explicit admission outcomes.
4. Provision one human seat and three separately owned automated consumers. Keep
   operator and bot credentials server-side; issue only the human's authorization
   through a deliberate account-to-seat adapter.
5. Compose the existing session recovery controller through an explicit host
   adapter, preserving Web Locks, exact command journaling and unknown outcomes.
   Complete and qualify all reachable browser decisions before claiming full play.
6. Persist owner-scoped game records and typed outcomes before adding real Home
   resume/history or result pages. Supervise bot lifetime and human think time;
   development SDK watchdogs and in-memory plans are not production supervision.

Compatibility states must preserve their meaning: not checked, checking,
supported, unsupported, unavailable and stale revision. Only authoritative
admission evidence can establish supported. Cookie sign-in alone grants no Rust
seat authority; the SDK currently uses a separate capability boundary.

## Verification

Check both real table consumers, package imports and scoped CSS, lazy game loading,
existing public-route bundle budget, direct page loads and reloads, normal and
empty accounts, unreadable deck selection, stale asynchronous deck switching,
keyboard/focus/draft continuity and desktop/tablet/phone navigation. Use disposable
source-backed browser fixtures. Keep real-host/full-game acceptance separate from
this scaffold and retain meaningful failed results.

## Current documentation entrypoints

- [Product setup](../packages/multiverse/README.md),
  [frontend architecture](../packages/multiverse/web/ARCHITECTURE.md) and
  [design system](../packages/multiverse/web/DESIGN.md) govern the application.
- [Backend manual](../packages/deck-builder/AGENTS.md),
  [operations](../packages/deck-builder/OPERATIONS.md) and
  [test ownership](../packages/deck-builder/TESTING.md) govern the existing service.
- [Client setup](../packages/game-client/README.md),
  [shared display manual](../packages/game-ui/AGENTS.md) and
  [delivery criteria](game-client-delivery.md) distinguish the preview/harness
  from future game launch.
- [Integration evidence](../packages/multiverse/INTEGRATION-REPORT.md) and
  [live opening evidence](../packages/game-client/LIVE-OPENING-REPORT.md) retain
  exact acceptance. Older release plans, audits and playtester briefs are
  historical context; they do not override current ownership or acceptance.
