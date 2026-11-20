# Oracle text parser

The hand-written TypeScript parser at `packages/oracle-parser` owns normalization,
syntax AST and diagnostic reference analysis. Rust owns supported complete-source
binding/lowering and executable Card IR; see [system architecture](../README.md#system-architecture)
and [catalog admission](game-engine-catalogs.md). The deck library has an independent
Scryfall/SQLite store. Broad grammar expansion is paused; parsing/reference binding
never certifies engine execution. This guide owns parser semantics, data/archive
contracts and ingestion; [the package manual](../packages/oracle-parser/AGENTS.md)
owns local development constraints.

## Pipeline and public API

```text
Oracle text → normalize → ability blocks → lex → parse → typed AST
                                                       ↓
                                           reference binding analysis
```

`parseOracleText(text, cardName?, context?)` returns the discriminated
`ParseCardResult` in [ast.ts](../packages/oracle-parser/src/ast.ts). Successful
cards have aggregate `abilities`; failures have only per-block `lines`, retaining
successful blocks and diagnostics. `lines[].text` is normalized, not an original
source span. The AST intentionally permits breaking changes, with no separate
implemented JSON Schema. Source types own the exact payload inventory.

`ok: true` means every block was consumed and represented by supported productions.
It is not semantic certification: structural tests/rules review must catch wrong
ASTs. Unknown syntax never becomes raw executable text.

## Responsibilities and invariants

- Normalize punctuation/reminder text and exact-case full/short self-references,
  preserving newlines even inside balanced parentheticals; malformed parentheses
  remain errors. Protect literal names in `named`, `Partner with` and named-token
  clauses. A complete-input `CardNameCatalog` supplies card/face/token names;
  grammar selects actual literals, rejecting unknown/ambiguous names.
- Lines ordinarily delimit abilities. Modal headers own all following bullet,
  Spree `+` or weighted `{P}` lines; consume every option or fail the whole block,
  and reject orphan bullets. An instruction-only self-replacement paragraph joins
  its preceding spell block and retains its ability word (CR 614.15).
- Lexing never drops unknown characters; costs, production and payment conditions
  validate their supported brace symbols. Parsers construct typed nodes directly,
  never `any` or lossy approximations. Returning `null` restores both cursor and
  elided-subject context through `Cursor.attempt`.
- Diagnostics come from the original classifier attempts. Sliced cost/loyalty/
  ability-word cursors retain full-header offsets; modal failures name option and
  local position. Farthest-failure token locations are not original character offsets.

## Data contracts

The [Scryfall adapter](../scripts/lib/scryfall.ts) validates consumed fields while
preserving unrelated fields. [Card objects](https://scryfall.com/docs/api/cards)
and [bulk data](https://scryfall.com/docs/api/bulk-data) distinguish printing `id`
from Oracle `oracle_id`; reversible faces carry Oracle identities. Missing/null
Oracle text differs from present empty text. `oracle_cards` selects representative
printings, not a complete set/printing index.

Corpus calls pass each face's `typeLine`, layout and complete `cardNames` catalog,
including token/excluded names without changing evaluation scope. Binding inputs
require `includeSourceProvenance: true`; the validator/archive enable it. The type
line prevents permanent text passing as a one-shot spell. Public product/backend
contracts and generated gameplay SDK types remain separate from this AST.

### AST archive

`npm run archive-parser` reads the local cache and writes ignored
`generated/oracle-ast.jsonl`; `--input PATH --output PATH` selects alternatives.
It never downloads or emits executable Card IR. [Archive code](../scripts/archive-parser.ts)
owns exact fields and serialization. The schema-3 manifest declares
`artifactKind: "oracle-ast-archive"`, `executable: false`, corpus byte/canonical-record
hashes, parser source hash/revision, rules edition, versioned scope, frozen paper
evidence identity and acceptance/exclusion/binding summaries. Source changes
during generation fail publication.

Adjacent content-addressed `corpus.snapshotFile` preserves exact original bytes,
including whitespace, numeric spellings and future fields. Decoded `source` is
convenient metadata, not the raw-input authority. `paperEvidence.snapshotFile`
preserves evidence bytes/hash/size and its complete printing download's hash,
size, count, URL and update time. Existing snapshots are verified, never overwritten;
missing/corrupt inputs block publication. Verification rechecks scope exclusions
against actual paper-printing witnesses, never names or format legality.

Each subsequent line retains one complete input record in original order:

- `source`, `sourceIndex`, `oracleIdentity`, `sourceSha256`: complete Scryfall object,
  every face/characteristic/unconsumed field and provenance.
- `excluded`: original source plus exclusion/duplicate reason, outside acceptance.
- `parsed`: all faces succeeded, full AST/normalized blocks and complete-card AST hash.
- `unsupported`: all available face results/diagnostics, no complete-card AST hash;
  missing/null text remains explicit `missing-oracle-text`, distinct from empty.

Selection IDs distinguish repeated use of one target from independent selections.
Original text stays in source; keywords and unresolved references remain symbolic.
In-scope rows retain `binding`/`bindingSha256`: graph, runtime slots, resolved edges
and diagnostics. `bindingSummary` separately counts reference-bound and fully-bound
cards, obligations/diagnostics and engine needs; these meanings are defined below.
Neither a parsed row nor a bound graph establishes executable support.

Publish verified source/evidence snapshots first, then write a unique adjacent
archive temporary file. Read every JSONL record back, verify exact serialization
and raw/source/AST/binding hashes before atomic replacement. Invalid/nonfinite
input, source edits or any verification failure preserve the prior archive;
ordinary failures remove only owned temporary outputs. Streaming verification
avoids a second whole-corpus allocation.

### Executable output boundary

Rust contracts own Card IR schema 1, evolved in place with strict capability/shape
validation until external compatibility requires otherwise. Archive schema 3 is
independent. `mtg-sim schema` and `catalog build MANIFEST [--write]` provide the
implemented schema/catalog path, under native-owner tooling. Bulk/per-set
`build-ir` remains unimplemented and fails explicitly. Future bulk output needs
complete printing/set membership, reproducible source/rules/corpus identity,
lossless faces/characteristics, strict binding and stable validated encoding;
Oracle corrections may rebuild existing sets. It is not inherently append-only.
Generated `packages/card-data/sets/` is never hand-edited/committed; engine catalogs
remain small reviewed source fixtures. Retired proposal detail is in Git history,
not another current schema contract.

## Semantic conventions

The foundational distinctions below are represented explicitly and covered by
regression tests. These are intentional breaking changes to the experimental
AST; no compatibility adapters are provided.

| Construct | Representation |
| --- | --- |
| `artifact creature` | `types: ["artifact", "creature"]`: both required |
| `artifact or creature` | `allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }] }]` |
| `red and green` / `red or green` | Conjunctive color array / explicit alternatives |
| Single-occurrence / grouped trigger | Default per occurrence / `grouping: "one-or-more"` |
| Graveyard event | `from` zone or `"anywhere"`, plus destination `ZoneRef` |
| Search | Acting player and complete searched zones, including their owners and any zone choice |
| Return to battlefield | Optional explicit `controller: "you" | "owner"` |
| Return to hand | Explicit `to: "your" | "owner"` |
| Activation restriction | Typed `sorcery` or `once-each-turn`, including loyalty abilities |
| Coordinated pump and ability gain | Shared trailing duration on both effects |
| Conditional self-replacement | Original instruction, replacement instructions, typed condition, and target-selection policy |
| Optional self-replacement | Explicit chooser; declining performs the original instruction |
| Abbreviated self-replacement | Unstated parts are the replaced instruction's own: damage recipient, token specification (`N of those tokens`), a pump's duration |
| Spell countered this way | `countered` destination on the counter event itself, replacing its owner's graveyard |
| Kicker alternative targets | Choose targets from the applicable casting branch, rather than announcing both branches |
| Conditional static grant | Condition scoped to each affected object when the wording refers to that object |
| Entering tapped, attached, attacking, or with counters | Modifiers of the entry event, rather than later instructions |
| Destruction with a no-regeneration rider | Regeneration prohibition on the destruction event itself |
| Explicit effect actor | Preserved independently of the selected objects and their owners |
| Spree / Tiered | Per-mode casting costs and optional labels, independently of each mode's resolution instructions |
| Weighted modes | Sum-of-weights budget, each mode's pawprint weight, and explicit repetition permission |
| Previously chosen modes | History scoped to the source object or the current turn, as printed |
| Explicit game outcome | `win-game` / `lose-game` with the affected player, independently of life changes |
| Personal-zone count condition | Current contents, full object filter, zone owner, and numeric comparison |
| Multi-zone search | One found-card selection across stated zones, zone-choice operator, owners, and separate search-event conditions |
| Aggregate quantities | Distinct characteristics, summed attributes, actual mana spent, and a scalar multiplier on a single event |
| Dice and result tables | Die kind, quantity, modifications, ignored dice, natural/final tests, complete ranges, and row instructions inside their original ability |
| Characteristic changes | Separate type/color/P/T components and intrinsic definitions versus ordinary granted modifications |
| Resolving characteristic changes | One `modify-characteristics` group retains the affected-object snapshot, type retention, colors, base P/T, and granted abilities |
| Characteristic relationships | Shared names/colors/type categories, scalar equality, and existential or universal comparison sets remain distinct; shared group constraints require a common value across the group |
| Counter-change triggers | Separate put/remove direction, counter kind, actor, counter/recipient grouping, threshold crossing, and last-counter removal |
| Named movement outcome | `put-action-history` queries the actual movement, actor, subject, destination, and instruction scope; it does not stand for an optional-cost decision |
| Optional placement fallback | An `otherwise` branch explicitly tests condition truth or whether the preceding optional placement happened |
| Activated keyword rules | Boast, exhaust, power-up, and forecast retain their intrinsic timing/history/cost rules through the typed keyword contract |
| Sequential library scan | Reveal/exile operation, player, owned library, required match count, and matching filter; the complete processed pool differs from the matching subset |
| Group selections | Collective attribute sums and pairwise differences constrain the selected group, separately from each candidate's eligibility |
| Existence of a qualifying group | `controls-subset` tests existence without selecting objects or counting the union of their names |
| Quantified instructions | `for-each` retains the member domain and instruction-wise processing, rather than executing the entire body for one player before the next |
| Hand reveal | The hand's owner is independent of the player instructed to reveal its cards |
| Ability recipient pronoun | Bare `they` remains a recipient whose player/object domain needs binding; parser order does not decide it |
| Main-phase ordinal | `second-main` is ordinal two in the current turn; `postcombat-main` includes every main phase after the first |
| Damage prevention | Cumulative budgets versus recurring/next-event prevention; source/recipient event grouping; shared versus individual shields; incoming versus actually prevented damage |
| Prevention variables | Resolving effects snapshot X when created; static prevention evaluates X when it applies to an event |
| Continuation after a conditional sentence | `afterConditional: "scope-unresolved"` on a sentence that refers back to what the preceding conditional sentence acted on; the parser does not decide whether the condition governs it |
| Bare "them" as a damage recipient | `to: "them"`: unresolved between a player and a set of objects, like `{ recipient: "they" }` |

Filter fields and ordinary array entries are conjunctive; only `anyOf` is
disjunctive. `allOf` combines nested clauses. The existing `orPlayer` field
represents a simple object-or-player union. Mixed color conjunctions without
an implemented unambiguous grouping are rejected.

Duration sharing is limited to supported coordinated modifiers with an elided
subject. It does not cross a `then` or sentence boundary. Other unsupported
wording must fail rather than invent scope.

The AST retains symbolic references such as `it`, `that-player`, `x`, and
keyword names. `src/semantic/bindings.ts` performs conservative reference
analysis over all parsed faces and source metadata. Its separate graph records
sources, ability scopes, target selections, event results, reference edges,
and unresolved obligations. `fullyBound` refers only to this reference analysis;
it is not semantic certification or engine execution support. Unsupported
capture scopes and ambiguous antecedents remain diagnostics.

A keyword line binds through `src/semantic/keyword-scopes.ts`. Each keyword
the parser produces has an entry: the CR 702 rule, the kinds of ability it
represents, and the player its printed parameters refer to. That player is
the activator for equip, the caster for kicker, the player taking the special
action for morph, and the controller of the targeting spell for ward. The
keyword's intrinsic targets and choices are fixed by the rules and are not
printed references. A keyword of another object, such as a created token,
has nothing to bind unless its parameters contain references; those still
need the other object's own scope. Companion's pregame deck condition is not
modeled.

Instructions nested in other instructions bind with explicit scope rules. A
"you may" instruction binds normally; its event records that it is optional and
which player decides (CR 608.2d), and declining it produces no results. An
optional group ("you may A, B, and C") performs its steps in order under one
decision. Each mode of a modal spell or ability binds in its own branch,
because a mode's targets exist only if it is chosen (CR 700.2c); nothing a
mode establishes is available to another mode or afterwards. A chosen
instruction ("+2/-2 or -2/+2") binds its options the same way, and a target
shared by the options is one selection. Simultaneous steps bind from the same
antecedents and all their results remain available. A "for each" rider is a
counted repetition, so how many objects it produces is not fixed. Modes that
can be chosen more than once, modes remembered across uses, created
replacement effects, repeated payments, and random outcomes keep their
unsupported-scope diagnostics.

Binding results have two levels. `referencesBound` means every reference on
the card resolves with no diagnostics. Some constructs need a rules-engine
capability that is not a binding question; they are recorded in
`engineFeatures` and their references bind as anywhere else:

- `event-history`: questions about what happened ("if you attacked this
  turn", "if this spell was cast from a graveyard", "creatures that attacked
  this turn", "the life you gained this turn"), asked of objects as they last
  existed (CR 400.7d, 608.2h-i). A condition about an earlier instruction of
  the same ability ("if you won the clash", "if [a player] did this way")
  remains a binding obligation.
- `damage-prevention`: prevention and redirection shields over future damage
  events (CR 609.7, 614.9, 615).
- `characteristic-layers`: resolving characteristic changes ("becomes a 3/3
  creature"), applied in the layer system with the affected set fixed when
  they begin (CR 611.2c, 613).

`fullyBound` means references bound and no engine feature needed.

Further scope rules, each grounded in the Comprehensive Rules:

- An ability an effect gives to objects ("Equipped creature has …", a quoted
  ability) is bound as an ability of the recipient: its "this creature" is
  each recipient and its "you" that object's controller (CR 113.10, 201.5,
  301.5d, 303.4e). A granted keyword's protection qualities and choices stay
  in the granting scope (CR 608.2h). "Equipped", "enchanted" and "fortified"
  objects are what the source is attached to (CR 301.5f, 303.4m).
- A permanent's own entry binds "you" to its entering controller (CR 110.2a,
  614.12). Optional same-turn `entry-copy-replacement` binds a pre-entry
  battlefield choice and needs engine support (CR 614.12a, 707.5).
- "An opponent controls" and "you don't control" are relative to "you".
  "Other" in a permanent's ability means other than that permanent when no
  other object has been named; elsewhere it stays unresolved. A bare "it"
  means the permanent under the same condition.
- Keyword actions bind their printed participants; their internal choices
  are fixed by CR 701. An action that creates or reveals objects adds them
  as candidates, so a later pronoun is never resolved past them.
- After an object leaves its zone, references to its controller, owner,
  characteristics, counters or past state ("if it was attacking") use its
  last-known information (CR 608.2h, 603.10a); a damage source that left
  uses its last-known information (CR 113.7a). Acting on it again means the
  object it became in a public zone: a zone-change trigger can find that
  object (CR 400.7e, 603.6c: "When this creature dies, return it to its
  owner's hand"), as can the rest of an effect that moved it there,
  including a discard (CR 400.7j). A card from a library or hand is known
  by its new zone's information. The card a trigger saw move, and a
  discarded card, are meant only when the effect names nothing else.
- An instruction's acting player and damage source are antecedents within
  that instruction ("Target opponent sacrifices a creature they control").
  A bare possessive means that subject ("this creature deals damage equal
  to its power to that creature"). The source, a definite trigger subject,
  and objects chosen by a cost are antecedents only when nothing else could
  be meant. An object an intervening "if" clause names as its subject (CR
  603.4), a static ability's "as long as" condition, or a sentence's
  condition is the antecedent of a bare pronoun it governs ("if that
  creature has power 4 or greater, it gains first strike"), unless the
  governed instructions introduce an object after the condition. A "this
  way" condition governs with the objects its instruction acted on. "That
  [noun]" means an antecedent established as that kind of object (CR 115.4
  for "any target"; cards from a library are cards, tokens are tokens,
  creatures and other permanent types are permanents) and never the object
  the text calls "this". The spell or ability that targeted an object is
  meant only by "that spell" or "that ability", never by "it".
- "That player" means a player named outright before one named only
  through an object ("its owner's hand", "a land an opponent controls",
  "a card an opponent owns"); never "you". A player targeted anywhere in an
  instruction ("search target player's library") and the player a trigger
  names ("at the beginning of enchanted player's upkeep") are antecedents.
  "Defending player" is the player the one attacking creature an ability
  is about attacks (CR 508.5); for several attacking creatures it stays
  unresolved (CR 508.5a). "You" on a card's own alternative cost is the
  player casting it, and on its own permission to be cast its owner (CR
  109.5). A trigger's other participant
  ("blocks a creature") is an antecedent; with alternative trigger events
  each binds on its own, and a shared other participant becomes "whichever
  the event established" (CR 603.2).
- Conditional sentences bind in their own branch; their results are not
  antecedents afterwards, but targets they announced are (CR 601.2c), and
  a sentence the parser marks as possibly governed by the previous
  condition is diagnosed. A condition printed after its instruction ("if
  its power is 2 or less", "unless its controller pays") is checked on that
  instruction's participants, not its results (CR 608.2c). "If you do",
  "When you do" link to the single optional instruction they follow; "If
  you don't" sees the antecedents from before that declined instruction.
  "Unless [player] pays" binds the payer to that player (CR 118.12a); its
  "for each ... you control" stays in the ability's voice (CR 109.5).
- An instant's or sorcery's paragraphs are one spell (CR 113.3a, 608.2c).
  Coin-flip outcomes and die-result rows are exclusive branches whose
  targets belong to the ability (CR 705, 706.3b).
- "That many" is the quantity of the instruction it is chained to in its
  sentence, or of the decision "if you do" refers to; an instruction whose
  own amount is "that many" only repeats it. Triggers measure damage, life
  paid, and how many objects a "one or more" event had; costs measure what
  they removed, paid or moved. X is announced for a spell or ability with X
  in a cost (CR 107.3a, 107.3k) and carried into the permanent's own entry
  (CR 107.3m); a printed definition wins (CR 107.3c).
- "Kicked" links to the one kicker ability on the object (CR 702.33e).
  "Destroyed this way" links to the one earlier destroy instruction of the
  ability visible from it; "the rest" is the pool the sentence chose from,
  less that choice. "Its controller's untap step" and "returns under its
  owner's control" (CR 610.3c) bind to each affected object's player.
- In a cast or activation trigger that does not name this object, a bare
  "it" with nothing else named is the spell or ability (CR 603.2c: "if no
  colored mana was spent to cast it, counter that spell"). "That creature"
  never means a card or a spell (CR 109.2). Copies and "each of that
  creature's colors" read the object as it last existed (CR 707.2, 608.2h),
  or as the permanent an effect put onto the battlefield.
- Delayed and reflexive triggered abilities have the creating ability's
  source and controller and may refer to the objects it named (CR 603.7c-e,
  603.12). A self-replacement binds its original and replacement
  instructions in separate branches; its condition is checked on the
  original's participants before it acts, and a replacement's pronoun means
  the original's participant in the same role (CR 614.15).

Pass `{ includeSourceProvenance: true }` as the third argument to
`parseOracleText` for binding inputs. Targets receive stable `selectionId`
values derived from the normalized source line and target token. Modal option
lines have distinct identities, and elided subjects preserve the original
selection identity. This metadata is optional for syntax-only use and required
to bind targeted selections; the corpus validator and archive always enable it.
Token IDs are not original Oracle character offsets.

Dice rows retain their source-line target identities. A result table is part of
the original ability, not a modal choice or a new triggered ability. Unresolved
roll occurrences, payment histories, collective quantities, and keyword history
scopes stay visible in binding diagnostics. They do not become fully bound just
because their syntax is supported.

For corpus parsing, also pass each face's `typeLine` and the shared `cardNames`
catalog. The type line prevents unsupported permanent text from passing as
one-shot spell instructions. Card-name lookup uses all source records,
including token and excluded records, without changing the validation scope.

General semantic lowering and linked-ability coverage remain unfinished. The Rust
compiler lowers only complete supported source; unresolved/unsupported constructs
must reject before executable emission. `build-ir` still fails explicitly.

Rules references: [Comprehensive Rules](https://magic.wizards.com/en/rules),
especially 107.4 (symbols), 602.5 (restrictions), 603.2c and 700.1 (events),
and 611.2a (durations). Parser comments cite the rules edition used for the
foundational fixes.

## Ingestion

These root scripts are independent of the deck-builder's ingestion/database.
`npm run ingest` requests the Scryfall bulk manifest with descriptive User-Agent/
Accept headers, selects `oracle_cards` and caches `.scryfall-cache/oracle-cards.json`.
Reuse a cache younger than 24 hours only after complete validation. Stream a new
download into a unique adjacent temporary file; validate then publish by
same-directory rename. HTTP/stream/JSON/record failures retain the prior corpus;
ordinary completion/failure removes the owned temporary file.

The reader supports current gzip JSONL manifests and historical JSON arrays.
Validate each JSONL line as exactly one card and frame an array without
reserializing fields/numeric spellings. Invalid UTF-8, truncated gzip or multiple
records on one printing-evidence line reject. Report/archive/evidence readers also
reject malformed UTF-8. Keep frozen benchmark inputs unchanged between comparisons;
downloads, generated reports and content-addressed snapshots stay ignored/preserved.

Validation/archive require `.scryfall-cache/paper-printing-evidence.json`;
ingestion does not create it. Current scope
`paper-printing-evidence-non-silver-non-acorn-v2` requires an eligible nonsilver,
nonacorn paper printing of each Oracle identity from source or frozen evidence.
A digital representative may identify a paper card; its `digital` flag alone does
not exclude that identity. Do not classify an entire Un-set as silver/acorn.
Exclude token, double-faced token, emblem, art-series, planar, vanguard and scheme
layouts; all other unsupported cards count as failures. Count exclusions separately,
faces independently and grouped modal blocks as single ability blocks. Empty text
is valid; missing/null face text fails.

Build paper evidence only from an explicitly downloaded, frozen `default_cards`
JSONL file and bulk manifest:

```sh
sh scripts/engine-env.sh node scripts/build-paper-evidence.ts default-cards.jsonl bulk-manifest.json .scryfall-cache/paper-printing-evidence.json
```

The builder refuses an existing output. It records one actual eligible printing
per identity and hashes the complete originating download. Reports hash evidence
separately from the unchanged Oracle corpus; missing evidence is an error, never
a smaller-denominator fallback. Historical v1 scope used the representative's
digital flag incorrectly and cannot supply comparable v2 percentages.

## Development and verification

Use the repository [.node-version](../.node-version) through
`sh scripts/engine-env.sh COMMAND`. Reuse healthy dependencies; when missing/broken,
follow [conditional engine/parser bootstrap](game-engine-development.md#fresh-worktree),
not root `npm install`. No Java or emitted parser build is required.

From repository root:

```sh
sh scripts/engine-env.sh npm --prefix packages/oracle-parser run check --workspaces=false
```

The gate runs TypeScript and structural/negative Node tests, including rollback,
meaning distinctions, complete-block failures and tooling/interrupted-download
fixtures. It needs no network or bulk cache. Fix false successes before extending
coverage; inspect exact source/rules and paired supported/rejection cases. Corpus
coverage is a separate check, run only when relevant:

```sh
sh scripts/engine-env.sh npm run validate
sh scripts/engine-env.sh npm run validate -- --json
sh scripts/engine-env.sh npm run archive-parser
```

Validation reads existing inputs without refreshing them. Reports bind exact
corpus/evidence/scope, Git revision and parser/reporting source hash including dirty
changes, with full failure examples and per-card acceptance/AST hashes. Compare
matching corpus/scope and review changed ASTs; reduced coverage may correct false
successes. Failure groups are investigation hints, not uniform grammar obligations.
Before reusing a report/archive, compare `parser.sourceSha256` to `sourceIdentity()`
in `scripts/validate-parser.ts`; recreate stale results only when needed. Never
infer current support from a filename such as `parser-98-*`.

## Source-qualified checkpoint and remaining work

Broad grammar work is paused. The 2026-10-16 checkpoint's 92.27% Commander-card
acceptance is historical, not current execution support. Exact parser source,
corpus/evidence pins, counts and earlier checkpoint11 audit locators remain in
[the immutable baseline](https://github.com/joCaldwell/mtg-ecosystem/blob/33dcbe9df5c0cc7fef5f8d67f97adb85a3af4cdb/docs/oracle_parser.md#source-qualified-checkpoint-and-remaining-work)
and [original checkpoint](https://github.com/joCaldwell/mtg-ecosystem/blob/1c3eca4c37b90de02decca316fefab5c8035140e/docs/oracle-parser-checkpoint.md).
Unique audits remain under `.scryfall-cache/{checkpoint11-review.json,checkpoint11-grammar-gain-audit-coverage.json,archive-checkpoint11-independent-audit.json}`;
no source/evidence was removed. Reused archives identify their checkpoint through
manifest hashes, not their path.

Outstanding work includes independent review of later binding rules, unresolved
reference groups, event-history/prevention/layer requirements, paused grammar and
bulk output. Earlier sampled review found three defects in 465 additions; later
rules relied on corpus diffs. Wrong binding is worse than an unresolved reference.
Rust complete-source lowering/admission establishes execution, not a parser score.
