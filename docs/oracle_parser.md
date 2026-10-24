# Oracle text parser

The parser is a hand-written, zero-runtime-dependency TypeScript package at
`packages/oracle-parser`. It normalizes Oracle text, tokenizes it, and builds a
structured AST. The ANTLR implementation was retired on 2026-08-01.

The parser remains a limited supported subset. The deck-builder is active and
standalone; it does not consume this AST. A narrow Rust scenario compiler and
runtime are implemented; general parser-to-executable compilation and bulk IR
emission remain unfinished. See [architecture.md](architecture.md) and the package
[AGENTS.md](../packages/oracle-parser/AGENTS.md).

The parser, ingestion, archive, and diagnostic reference analysis remain
TypeScript. Production semantic binding, lowering, and Card IR emission belong
to the Rust compiler alongside the rules engine; see the
[adopted language decision](runtime-language-decision.md).

The [checkpoint history](oracle-parser-checkpoint.md) records verified coverage,
reproduction inputs, and unfinished work, including the October 15–16 parsing
and reference-binding results.

## Pipeline and public API

```text
Oracle text → normalize → ability blocks → lex → parse → typed AST
                                                       ↓
                                           reference binding analysis
                                                       ↓ future
                                         semantic validation/lowering → Card IR
```

```typescript
import { parseOracleText } from "@mtg-ecosystem/oracle-parser";

const result = parseOracleText("Flying\n{T}: Add {G}.");
if (result.ok) {
  // result.abilities is required here: KeywordLine, ActivatedAbility.
  console.log(result.abilities);
} else {
  for (const line of result.lines) {
    if (!line.ok) console.error(line.text, line.error);
  }
}
```

`src/ast.ts` is the authoritative current contract. `ParsedLine` and
`ParseCardResult` are discriminated success/failure unions. A failed card has
no aggregate `abilities`; successfully parsed individual blocks remain
available in `lines` for diagnostics and coverage work.

`ok: true` means every normalized ability block was consumed by supported
productions and represented in the AST. It is not proof of semantic correctness
or a promise that a downstream engine implements the represented mechanics.
Unknown syntax must fail rather than be preserved as raw executable text.
Acceptance statistics cannot detect an incorrect AST; structural tests and
rules review are the correctness gate.

## Responsibilities and invariants

- **Normalization** canonicalizes punctuation, removes parenthetical reminder
  text, replaces exact-case full/short self-references, and preserves newlines.
  Input is assumed to be Oracle text. `lines[].text` is normalized text, not an
  original-source span. Balanced nested parentheticals preserve line boundaries;
  malformed parentheses remain errors. Literal names in `named`, `Partner
  with`, and named-token creation clauses are protected from object-self
  substitution. A `CardNameCatalog` built from the complete input snapshot
  supplies canonical card, face, and token names; grammar decides which
  catalog matches are actual name literals. Unknown or ambiguous names fail.
- **Ability blocks** ordinarily follow lines. Modal headers own all immediately
  following bullet, Spree `+`, or weighted `{P}` lines. A successful header must actually consume those
  options. One failed option fails the entire block; orphaned bullets fail.
  A paragraph that only replaces the preceding spell ability's instruction
  (CR 614.15, "Morbid — ~ deals 5 damage instead if …") joins that ability's
  block; its ability word is kept on the `self-replacement` node.
- **Lexing** never drops unknown characters. Brace tokens remain generic game
  symbols; costs, mana production, and payment conditions validate their own
  supported symbols.
- **Parsing** constructs nodes directly. No generated grammar or visitor exists.
  A cursor parser returning `null` restores its starting position. Effect
  parsers also restore elided-subject context when an attempt fails.
- **Diagnostics** come from the original classifier attempts. Sliced cost,
  loyalty, and ability-word cursors report offsets in the complete header;
  failed modal options report their option number and local token position.
  Backtracking expectations can still be noisy; token positions are not
  original-text character offsets.

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
- A permanent's own entry ("enters tapped", "enters with counters", "as this
  enters") binds "you" to the player it enters under (CR 110.2a, 614.12).
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

Semantic validation, linked-ability support, and execution lowering remain
unfinished. They must reject unresolved or unsupported constructs before
producing executable Card IR. `build-ir` continues to fail explicitly.

Rules references: [Comprehensive Rules](https://magic.wizards.com/en/rules),
especially 107.4 (symbols), 602.5 (restrictions), 603.2c and 700.1 (events),
and 611.2a (durations). Parser comments cite the rules edition used for the
foundational fixes.

## Development and verification

Use Node ≥ 23 and run `npm install` at the repository root. There is no Java
requirement or emitted build. From `packages/oracle-parser`:

```sh
npm run check
```

This runs TypeScript checking and Node's structural test suite, including
parser rollback, meaning-distinction tests, complete-block failures, corpus
reporting, and simulated interrupted downloads. Pipeline modules imported by
tests are typechecked too. Tests do not require networking or the bulk cache.

From the root:

```sh
npm run ingest
npm run validate
npm run validate -- --json > .scryfall-cache/validation.json
npm run archive-parser
```

Validation uses the existing cache without refreshing it. Reports identify the
exact corpus by SHA-256, the Git revision, and a hash of parser and reporting
sources including working changes. Per-card outcomes include successful AST
hashes for comparing runs. Use the same corpus hash and scope when comparing;
changed AST hashes need review and are not automatically regressions.

Ingestion accepts the current Scryfall gzip JSONL bulk manifest as well as
historical JSON-array downloads. JSONL records are validated and framed as an
array without reserializing their fields or numeric spellings. Invalid UTF-8,
truncated gzip, malformed records, and interrupted downloads leave the previous
corpus intact. Benchmark work keeps its existing cache frozen instead of running
ingestion between coverage checkpoints.

Report, archive, and printing-evidence readers also reject invalid UTF-8.
Each printing JSONL line must contain exactly one complete card; a second
record on the same line cannot be silently omitted from the evidence.

The current scope, `paper-printing-evidence-non-silver-non-acorn-v2`, requires
an eligible paper printing of the Oracle identity, either the selected source
record or a printing in the frozen `.scryfall-cache/paper-printing-evidence.json`.
An Oracle bulk record can be an MTGO printing of a paper card; its `digital`
flag alone cannot exclude the identity. Evidence records one actual nonsilver,
nonacorn paper printing per identity and hashes the complete printing download
from which it was derived. Reports hash this evidence separately from the
unchanged Oracle text corpus. Missing evidence is an error, not permission to
fall back to a smaller denominator.

To rebuild evidence from an explicitly downloaded and frozen Scryfall
`default_cards` JSONL file and its bulk manifest:

```sh
node scripts/build-paper-evidence.ts default-cards.jsonl bulk-manifest.json .scryfall-cache/paper-printing-evidence.json
```

The builder refuses to overwrite an existing evidence file. Archive generation
also stores its exact bytes as a content-addressed snapshot. The historical
v1 scope incorrectly used the representative's digital flag; checkpoints made
with it are historical measurements, not comparable v2 percentages.

The scope still excludes explicitly listed non-card layouts. Other unsupported cards remain failures. Exclusions
are counted separately. Identity uses `oracle_id` (face IDs for reversible
cards), not names or printing IDs. Empty rules text is valid; missing or null face text
is reported as a failure. “Lines” count ability blocks, including grouped modes.

The old rewrite baseline was 33% of cards / 53% of lines under a different
population definition. It is historical, not a minimum acceptance target.
Correcting false successes can lower coverage. Failure groups use the token at
the failure location and retain complete example text; they are investigation
hints, not proof that every example needs the same grammar extension.

## Next milestones

1. Keep all known meaning-loss regressions fixed or explicitly rejected. Review
   newly accepted ASTs against card data and the rules.
2. Expand supported constructs with full structural assertions and negative
   tests; pair near-identical inputs when a word changes the meaning.
3. Extend the conservative reference graph to linked abilities and nested
   captures, then expand semantic validation, keyword expansion, and executable
   admission beyond the existing Rust scenario compiler/runtime.
4. Implement bulk emission only after the admitted subset passes validation
   end to end. Corpus coverage alone is not an emission gate.

`npm run build-ir` deliberately fails. The proposed envelope and remaining
storage decisions are described in [data_schemas.md](data_schemas.md).
