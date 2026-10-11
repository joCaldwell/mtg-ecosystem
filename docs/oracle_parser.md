# Oracle text parser

The parser is a hand-written, zero-runtime-dependency TypeScript package at
`packages/oracle-parser`. It normalizes Oracle text, tokenizes it, and builds a
structured AST. The ANTLR implementation was retired on 2026-08-01.

The parser remains a limited supported subset. The deck-builder is active and
standalone; it does not consume this AST. A rules engine and IR emitter are not
implemented. See [architecture.md](architecture.md) and the package
[AGENTS.md](../packages/oracle-parser/AGENTS.md).

The [2026-10-10 checkpoint](oracle-parser-checkpoint.md) records the stopping
point, verified coverage, reproduction inputs, and unfinished work.

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
   captures, then implement semantic validation, keyword expansion, and the
   capability boundary with a first engine consumer.
4. Implement an emitter only after a small, audited subset can pass that
   validation end to end. Corpus coverage alone is not an emission gate.

`npm run build-ir` deliberately fails. The proposed envelope and remaining
storage decisions are described in [data_schemas.md](data_schemas.md).
