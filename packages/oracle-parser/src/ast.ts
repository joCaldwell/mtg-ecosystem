// Typed AST for parsed oracle text.
//
// Design rules:
//  - Discriminated unions everywhere; no `any`. If a construct can't be
//    represented honestly, the parser must FAIL the line, not shoehorn it.
//  - Nodes describe what the rules text *means* (per the Comprehensive
//    Rules), not how it happens to be worded. Wording variants that mean
//    the same thing produce the same node.
//  - Every parse is per-line: one oracle-text line = one Ability (modal
//    bullet lines are folded into their parent Modal effect).

// ---------------------------------------------------------------------------
// Card / line results
// ---------------------------------------------------------------------------

/** Success means the whole normalized ability block is represented; it is not
 * a certificate that a future rules engine implements every represented node. */
export type ParsedLine =
  | { text: string; ok: true; ability: Ability }
  | { text: string; ok: false; error: string };

export type ParseCardResult =
  | { name: string; ok: true; lines: ParsedLine[]; abilities: Ability[] }
  | { name: string; ok: false; lines: ParsedLine[] };

// ---------------------------------------------------------------------------
// Abilities (one per line)
// ---------------------------------------------------------------------------

export type Ability =
  | KeywordLine
  | ActivatedAbility
  | LoyaltyAbility
  | TriggeredAbility
  | StaticAbility
  | SpellLine
  | { kind: "chapter"; chapters: number[]; effects: Sentence[] }
  | { kind: "max-speed"; ability: Ability }
  /** CR 701.43d: "You may exert [this creature] as it attacks" is an optional
   * cost to attack (508.1g). The "When you do" ability printed in the same
   * paragraph is linked to it and triggers only if exerted; `reflexive` is its
   * resolution text, where "it" is the exerted permanent. */
  | { kind: "exert-on-attack"; what: ObjectRef; reflexive?: Sentence[] }
  /** CR 716.2: one printed level bar represents both an activation and a
   * static grant. The activation pays these costs, requires current Class
   * designation level-1 and sorcery timing, and sets the designation to
   * level on resolution. The source has the nested abilities on battlefield
   * while its designation is at least level. The bar and all its contents
   * are copiable; the current designation is not. No designation means 1,
   * and a designation persists even if the permanent ceases to be a Class.
   * Class designations have no interaction with level counters. */
  | { kind: "class-level-bar"; level: number; costs: Cost[]; abilities: Ability[] }
  /** CR719.3a: at the beginning of your end step, if condition AND this
   * permanent is not solved, it becomes solved. Both clauses are checked
   * when the ability would trigger and again on resolution (603.4).
   * Solved is a designation, not an ability or a copiable value; it persists
   * until the permanent leaves the battlefield (719.3b). */
  | { kind: "case-solve"; condition: Condition }
  /** CR702.169b-d: a static child applies while this source is
   * solved; a triggered child checks solved only when it would trigger;
   * an activated/loyalty child requires solved to activate. This is never
   * an intervening-if clause on the child, nor a check during resolution.
   * In particular, sacrificing the solved Case as a cost is permitted. */
  | { kind: "case-solved"; ability: StaticAbility | TriggeredAbility | ActivatedAbility | LoyaltyAbility | SolvedKeywordLine }
  | AdditionalCostLine;

/** "Flying, first strike, ward {2}" */
export interface KeywordLine {
  kind: "keywords";
  keywords: KeywordInstance[];
}

/** The keyword's intrinsic class determines its Solved gate, not its
 * abbreviated printing: Equip checks activation and Ward checks triggering.
 * This does not remove/grant the keyword when the designation changes, and
 * neither gate is rechecked on resolution. Mixed-component keywords need
 * separate component gates and are not represented by this single gate. */
export interface SolvedKeywordLine {
  kind: "keywords";
  keywords: (KeywordInstance & { solvedGate: "continuous" | "trigger-time" | "activation" })[];
}

/** "{T}, Sacrifice ~: Draw a card." */
export interface ActivatedAbility {
  kind: "activated";
  abilityWord?: string;
  costs: Cost[];
  effects: Sentence[];
  restriction?: ActivationRestriction;
  /** CR 602.1b: payment instructions, not part of the resolving effect. */
  costModifications?: CostModification[];
  /** Keyword governing activation, including its complete intrinsic rules.
   * Power-up (702.193a/b): activate only once per source incarnation. If the
   * permanent entered this turn, reduce this ability's cost by its mana cost:
   * generic reduces generic; colored/colorless reduces matching types, with
   * excess reducing generic (118.7). It imposes no sorcery timing restriction.
   * This is dynamic payment information, not a rewrite of the printed cost.
   * Forecast (702.57a/b): activate only from the owner's hand, only during
   * their upkeep and once each turn. The activator reveals the source as
   * activated and keeps it revealed until it leaves their hand or a step or
   * phase other than an upkeep begins. Printed costs remain separate. */
  keyword?: "boast" | "exhaust" | "power-up" | "forecast";
  permission?: { activators: "any-player" | "opponents-of-source-controller" };
}

/** CR 602.5: restrictions are semantic instructions, never raw accepted text.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt */
export type ActivationRestriction =
  | { restriction: "sorcery" }
  | { restriction: "instant" }
  | { restriction: "once-each-turn" }
  | { restriction: "activation-limit"; maximum: number; period: "turn" | "object" }
  | { restriction: "timing"; turn?: TurnOwner | "activator"; phase?: PhaseName; before?: "declare-attackers" | "end-step" | "end-of-combat" }
  | { restriction: "condition"; condition: Condition }
  | { restriction: "all-of"; restrictions: ActivationRestriction[] };

/** "+1: …", "−X: …", "0: …" */
export interface LoyaltyAbility {
  kind: "loyalty";
  cost: { sign: 1 | -1 | 0; amount: number | "x" };
  effects: Sentence[];
  restriction?: ActivationRestriction;
  costModifications?: CostModification[];
  permission?: { activators: "any-player" | "opponents-of-source-controller" };
}

/** "Whenever ~ attacks, draw a card." */
export interface TriggeredAbility {
  kind: "triggered";
  abilityWord?: string;
  trigger: Trigger;
  /** Intervening "if" clause: "When ~ dies, if it had a counter on it, …" */
  condition?: Condition;
  effects: Sentence[];
  /** Limits actual triggers, independently of matching-event ordinals. */
  limit?: TriggerLimit;
  /** CR 603.2h "Do this only once each turn": the ability triggers only if
   * its controller has not yet taken the indicated action this turn. A
   * declined action does not count, unlike `limit`, which counts triggers.
   * "This" is the ability's own instruction containing the optional action;
   * identifying that action among several is left to binding. */
  actionLimit?: { maximum: 1; period: "turn" };
}

export interface TriggerLimit { maximum: number; period: "turn" | "ability-instance" }

/** Continuous effects: "Creatures you control get +1/+1." */
export interface StaticAbility {
  kind: "static";
  abilityWord?: string;
  effect: StaticEffect;
}

/** Imperative resolution text (instants/sorceries, or trigger bodies). */
export interface SpellLine {
  kind: "spell";
  abilityWord?: string;
  effects: Sentence[];
}

/** "As an additional cost to cast this spell, sacrifice a creature." */
export interface AdditionalCostLine {
  kind: "additional-cost";
  costs: Cost[];
  /** CR 118.8b, 601.2b: "you may" — the caster announces whether to pay it. */
  optional?: true;
  /** Printed with the cost: adjusts this spell's own total cost (601.2f). */
  costModifications?: CostModification[];
}

// ---------------------------------------------------------------------------
// Keywords
// ---------------------------------------------------------------------------

export interface KeywordInstance {
  /** Canonical lowercase keyword name, e.g. "flying", "first strike", "cycling". */
  keyword: string;
  /** For cost-parameterized keywords: Equip {2}, Ward—Pay 3 life. */
  cost?: Cost[];
  /** Explicit activation-payment instructions attached to this keyword. */
  costModifications?: CostModification[];
  /** Printed restriction on this keyword's activated ability, in addition
   * to the keyword's own rules (CR 602.5b): "Equip {0}. Activate only once
   * each turn." */
  restriction?: ActivationRestriction;
  /** For number-parameterized keywords: Crew 2, Toxic 1, Fading 3. */
  amount?: number | "x";
  /** Protection from …, hexproof from … */
  from?: ProtectionScope[];
  /** Enchant creature, Affinity for artifacts, …walk */
  filter?: ObjectFilter;
  /** Partner with <name> */
  pairedWith?: string;
  enchantPlayer?: "player" | "opponent";
  prototype?: { power: Amount; toughness: Amount };
  /** Multiple independently payable kicker costs (CR 702.33). */
  additionalCosts?: Cost[][];
  gift?: "card" | "tapped-fish" | "treasure" | "food" | "octopus" | "extra-turn";
}

export type ProtectionScope =
  | { scope: "color"; color: Color }
  | { scope: "chosen-value"; value: ChosenValueRef }
  /** One fresh choice for this occurrence of the granting instruction (shared
   * by its recipients). Its chooser is evaluated in the granting instruction's
   * scope, not the gained keyword's source scope. This is not a linked earlier
   * choice or an intrinsic keyword. CR 608.2d, 702.16. */
  | { scope: "fresh-choice"; category: "color" | "card-type"; who: PlayerRef; when: "granting-effect-applies" }
  /** CR 702.16i: protection from each color in this set. Resolving effects
   * evaluate the set once (608.2h); static effects reevaluate it (611.3a). */
  | { scope: "colors-of"; what: ObjectRef }
  | { scope: "colorless" }
  | { scope: "multicolored" }
  | { scope: "monocolored" }
  | { scope: "all-colors" }
  | { scope: "everything" }
  | { scope: "filter"; filter: ObjectFilter }
  | { scope: "player"; player: PlayerRef };

// ---------------------------------------------------------------------------
// Costs
// ---------------------------------------------------------------------------

export type Cost =
  | { cost: "mana"; symbols: string[] }              // ["2","G","G"] or ["X"] or ["W/P"]
  | { cost: "tap-self" }                             // {T}
  | { cost: "untap-self" }                           // {Q}
  | { cost: "energy"; amount: Amount }               // {E}{E}, Pay X {E}
  | { cost: "pay-life"; amount: Amount }
  | { cost: "sacrifice"; what: ObjectRef }
  | { cost: "discard"; what: ObjectRef | "hand"; random?: boolean }
  | { cost: "reveal"; what: ObjectRef; from?: ZoneRef }
  | { cost: "choice"; options: Cost[][] }
  | { cost: "exile"; what: ObjectRef; from?: ZoneRef }
  | { cost: "tap-objects"; what: ObjectRef }         // "Tap two untapped Allies you control"
  | { cost: "untap-objects"; what: ObjectRef }
  | { cost: "exert"; what: ObjectRef }
  | { cost: "mill"; amount: Amount }
  | { cost: "return"; what: ObjectRef; to: "hand"; owner: "your" | "owner"; from?: ZoneRef }
  | { cost: "remove-counters"; counter: CounterSpec | "any"; count: Amount | "all"; from: ObjectRef; distributed?: true }
  | { cost: "put-counters"; counter: CounterSpec; count: Amount; on: ObjectRef }
  | { cost: "collect-evidence"; amount: Amount }
  | { cost: "waterbend"; symbols: string[] }
  | { cost: "forage" }
  | { cost: "blight"; amount: Amount }
  | { cost: "get-counters"; counter: CounterSpec; count: Amount };

// ---------------------------------------------------------------------------
// Triggers
// ---------------------------------------------------------------------------

/** Omitted grouping means each matching occurrence; one-or-more means once
 * for the matching group in an event (CR 603.2c, 700.1). */
export type Trigger = {
  grouping?: "one-or-more";
  controller?: PlayerRef;
  /** `per: "player"` ("their second spell each turn") counts separately for
   * each player performing the event; otherwise the matching events of the
   * period are counted together. */
  occurrence?: { ordinal: number; period: "turn"; per?: "player" };
  during?: TurnOwner;
} & (
  | { trigger: "any-of"; events: Trigger[] }
  /** CR603.1/4/10: a "while" qualification is checked only against the
   * matching event's state/snapshot, not again as this ability resolves.
   * The wrapper qualifies only this event branch. A subject-status gate
   * tests the object that participates in that occurrence; it is not a
   * freshly selected object or a current board-wide filter at resolution. */
  | { trigger: "event-gated"; event: Trigger; gate: TriggerEventGate }
  /** CR 603.8: a state trigger. It triggers as soon as the game state matches
   * the condition and not again until this ability has left the stack. This
   * is neither an event nor an intervening-if clause (603.4). */
  | { trigger: "state"; condition: Condition }
  /** untapped restricts the entry event itself (CR 603.6a, 614.12); it is
   * not a test of the permanent's status when the ability resolves. */
  | { trigger: "enters"; what: ObjectRef; from?: ZoneRef; untapped?: true }
  /** withoutDying: any destination except a graveyard (CR 700.4). */
  | { trigger: "leaves"; what: ObjectRef; withoutDying?: true }
  | { trigger: "leaves-zone"; what: ObjectRef; from: ZoneRef }
  | { trigger: "dies"; what: ObjectRef }
  | { trigger: "enters-or-leaves"; what: ObjectRef }
  | { trigger: "enters-or-attacks"; what: ObjectRef }
  | { trigger: "enters-or-dies"; what: ObjectRef }
  /** notFrom ("from anywhere other than") excludes one origin zone. */
  | { trigger: "put-into-graveyard"; what: ObjectRef; from: Zone | ZoneRef | "anywhere"; to: ZoneRef; notFrom?: ZoneRef }
  /** A move into exile, a library, a hand or the command zone. An omitted
   * origin was not printed; it is never inferred from the subject. From the
   * battlefield this is a leaves-the-battlefield ability (CR 603.6c). */
  | { trigger: "put-into-zone"; what: ObjectRef; from?: ZoneRef | "anywhere"; to: ZoneRef }
  | { trigger: "cast"; who: PlayerRef; what: ObjectFilter | ObjectRef; from?: ZoneRef }
  /** CR 701.18a/b: playing a land is the special action, not a land entering
   * by an effect; playing a card is that land play or casting it as a spell.
   * from is the zone the card was played from, not its zone afterwards. */
  | { trigger: "plays"; who: PlayerRef; what: ObjectFilter | ObjectRef; from?: ZoneRef }
  /** alongside: the same declaration also contains this many other attackers
   * matching the filter, besides the subject (CR 508.1, 508.3a). */
  | { trigger: "attacks"; what: ObjectRef; alone?: boolean; whom?: (PlayerRef | ObjectRef)[];
      alongside?: { filter: ObjectFilter; count: Comparison } }
  | { trigger: "you-attack" }
  /** CR 508.3c–e: "with a creature" triggers for each matching attacker;
   * "with one or more" triggers once per declaration (or attacked player).
   * count measures the qualifying attackers in that single declaration. */
  | { trigger: "player-attacks"; who: PlayerRef; with?: ObjectFilter; count?: Comparison;
      whom?: PlayerRef; per: "attacker" | "declaration" }
  | { trigger: "blocks"; what: ObjectRef; orBecomesBlocked?: boolean; blocking?: ObjectRef }
  | { trigger: "becomes-blocked"; what: ObjectRef; by?: ObjectRef }
  | { trigger: "becomes-target"; what: ObjectRef; of: ObjectFilter }
  | { trigger: "becomes-tapped" | "becomes-untapped"; what: ObjectRef }
  | { trigger: "becomes-status"; what: ObjectRef; status: "monstrous" | "renowned" }
  /** CR 716.2: transition to this exact Class designation, not entering
   * the battlefield, obtaining level counters, or merely having the level. */
  | { trigger: "becomes-class-level"; what: ObjectRef; level: number }
  | { trigger: "activate"; who: PlayerRef; what: ObjectFilter }
  /** CR122.2/6/7,603.2c: actual counter changes, including entry with
   * counters. Counters disappearing on a zone change are not removals.
   * `by` is the player performing the change, only constrained if printed;
   * it is never inferred from the recipient's owner or controller.
   * The event quantity is the number of matching counters in this trigger
   * occurrence, not the recipient's current count. Matching is unaffected
   * by how many counter kinds an unconstrained (`any`) event contains. */
  | { trigger: "counter-change"; direction: "put" | "remove"; counter: CounterSpec | "any";
      on: CounterChangeRecipient; by?: PlayerRef; match: CounterChangeMatch }
  /** Omitted die/result matches dice including the planar die (706.7).
   * Numeric result tests never include planar rolls. Grouping is per roll
   * unless the printed event says one or more dice. Ignored rolls do not
   * exist for trigger purposes (706.6). */
  | { trigger: "roll-dice"; who: PlayerRef; die?: NumericDie; result?: DiceResultTest }
  | { trigger: "cycle"; who: PlayerRef; what: ObjectRef }
  | { trigger: "copy"; who: PlayerRef; what: ObjectFilter }
  | { trigger: "turned-face-up" | "mutates"; what: ObjectRef }
  | { trigger: "transforms"; what: ObjectRef; into?: { face: "source-face" } }
  | { trigger: "phase"; phase: PhaseName; whose: TurnOwner | PlayerRef }
  /** A phase/step in one identified turn, not the next such step regardless
   * of turn. Used by delayed abilities; extra upkeeps in the current turn
   * cannot satisfy a next-turn upkeep (CR603.7b,614.10a). */
  | { trigger: "phase-in-turn"; phase: Exclude<PhaseName, "turn">; turn: TurnRef }
  /** The beginning of one contextually identified combat. An intervening
   * inserted combat must not become its antecedent merely by being next. */
  | { trigger: "phase-instance"; phase: { phase: "that"; kind: "combat"; scope: "contextual" }; boundary: "beginning" }
  /** `to: "any"` denotes the printed event recipient "a player" (each
   * matching player occurrence), not an arbitrary damageable object. */
  /** combat: true is combat damage only, false is noncombat damage only,
   * omitted is any damage (CR 120.2). */
  | { trigger: "deals-damage"; what: ObjectRef; combat?: boolean; to?: PlayerRef | ObjectRef | "any" }
  | { trigger: "is-dealt-damage"; what: ObjectRef; combat?: boolean }
  /** The printed event names only the damaged player and no source, so the
   * occurrence is the player being dealt damage (CR 120.3a/b, 603.2c); it
   * is not rewritten as a per-source deals-damage event. */
  | { trigger: "player-dealt-damage"; who: PlayerRef; combat?: boolean }
  | { trigger: "gains-life"; who: PlayerRef }
  | { trigger: "loses-life"; who: PlayerRef }
  | { trigger: "draws"; who: PlayerRef }
  | { trigger: "discards"; who: PlayerRef; what?: ObjectRef }
  | { trigger: "sacrifices"; who: PlayerRef; what: ObjectFilter | ObjectRef }
  | { trigger: "scries" | "surveils"; who: PlayerRef }
  /** CR 702.110b: the creature's controller sacrifices a creature as its
   * exploit ability resolves. exploited describes the sacrificed creature. */
  | { trigger: "exploits"; what: ObjectRef; exploited: ObjectRef }
  /** CR 700.13: the player casts a spell, activates an ability or puts a
   * triggered ability on the stack that targets an opponent, something an
   * opponent controls, or a card in an opponent's graveyard. */
  | { trigger: "commits-crime"; who: PlayerRef }
  /** CR 700.14: the total mana this player spent to cast spells this turn
   * was below amount and reaches at least amount with this payment. */
  | { trigger: "expends"; who: PlayerRef; amount: number }
  /** CR 709.5h/j: the permanent gets the unlocked designation of the half
   * (door) this ability is printed on, on entering or afterwards. */
  | { trigger: "unlocks-door"; who: PlayerRef; door: "this" }
  /** Contraption rules are outside the Comprehensive Rules (CR 701.45a
   * defers to the Unstable FAQ); this names the printed crank event only. */
  | { trigger: "cranks"; who: PlayerRef; what: ObjectRef }
  /** CR 106.12/106.12a: a mana ability with {T} in its cost resolves and
   * produces mana. who, if printed, is the player who activated it. */
  | { trigger: "taps-for-mana"; what: ObjectRef; who?: PlayerRef });

export type TriggerEventGate =
  | { gate: "condition"; condition: Condition }
  | { gate: "subject-status"; status: "saddled" };

export type CounterChangeSingleRecipient =
  | { recipient: "object"; what: ObjectRef }
  | { recipient: "player"; who: PlayerRef };
/** Alternatives are eligible recipients, not multiple separate changes.
 * Their cardinality/grouping is specified by CounterChangeMatch. */
export type CounterChangeRecipient = CounterChangeSingleRecipient
  | { recipient: "any-of"; recipients: [CounterChangeSingleRecipient, CounterChangeSingleRecipient, ...CounterChangeSingleRecipient[]] };
export type CounterChangeMatch =
  /** One occurrence per matching counter on each recipient. */
  | { counters: "each"; recipients: "each" }
  /** Per-recipient grouping is distinct from one occurrence across all
   * eligible recipients in a simultaneous event. In both cases filter the
   * counter kinds and recipients before grouping (CR603.2c,700.1). */
  | { counters: "one-or-more"; recipients: "each" | "one-or-more" }
  /** Before < threshold and after >= threshold; not an exact final count
   * or an ordinal in the lifetime history of additions (CR122.7). */
  | { transition: "crosses-upward"; threshold: number }
  /** An actual removal that changes a positive matching count to zero. */
  | { transition: "last-removed" };

export type PhaseName =
  | "upkeep" | "draw-step" | "untap-step" | "combat" | "declare-attackers"
  | "declare-blockers" | "combat-damage" | "end-of-combat" | "end-step"
  // CR505.1a/b: every main phase after the first is postcombat; "second"
  // is specifically ordinal 2 in the current turn, not every such phase.
  | "cleanup" | "main" | "precombat-main" | "second-main" | "postcombat-main" | "turn";

export type TurnOwner = "your" | "each-player" | "each-opponent" | "that-player" | "active-player" | "opponent";
/** A particular turn occurrence. Absent next-turn owner means the next turn
 * in the game's schedule, not the ability controller's next turn. A `that`
 * reference requires a contextual turn-event binding, never a player/object
 * antecedent or an invented most-recent-added-turn heuristic. */
export type TurnRef =
  | { turn: "current" }
  | { turn: "next"; whose?: PlayerRef }
  | { turn: "that"; scope: "contextual" };

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

/** One sentence of resolution text: connected effects plus an optional leading condition. */
export interface Sentence {
  /** "If you control a Dragon, …" / "… unless its controller pays {2}." */
  condition?: Condition;
  conditionKind?: "if" | "unless";
  /** Effects joined by "," / "then" / "and" — executed in order. */
  steps: Effect[];
  /** A following "Otherwise" can test the condition or the actual outcome
   * of the sole optional action. action-not-performed also covers failing
   * the condition, declining the action, or being unable to carry it out. */
  otherwise?: { on: "condition-false" | "condition-true" | "action-not-performed"; steps: Effect[] };
  /** Explicit definitions in this instruction, e.g. "where X is its power". */
  bindings?: { variable: "x"; value: Amount }[];
}

interface EffectBase {
  /** "You may …" */
  optional?: true;
  /** "… for each Elf you control." */
  forEach?: ObjectFilter;
  /** "… for each death vote." The effect is repeated once per vote cast for
   * that choice by the preceding vote (CR 701.38). */
  forEachVote?: { choice: string };
  /** "… until end of turn." */
  duration?: Duration;
}

/** Inclusive result ranges are not clamped to a die's natural faces: numeric
 * modifiers can produce values below 1 or above its side count (CR 706.2/3). */
export type DiceRange =
  | { range: "exact"; value: number }
  | { range: "closed"; minimum: number; maximum: number }
  | { range: "at-least" | "at-most"; value: number }
  | { range: "any-of"; ranges: DiceRange[] };
export interface NumericDie { die: "numeric"; sides: number }
export type DieSpec = NumericDie | { die: "planar" };
export type DiceResultTest = { basis: "natural" | "final"; range: DiceRange }
  | { basis: "natural"; highest: true };
export interface DiceResultTable {
  /** Rows are conditional instructions, never modes or independent abilities.
   * Target declarations still belong to the original ability (706.3b). */
  rows: { range: DiceRange; effects: Sentence[]; label?: string }[];
}

/** A complete temporal occurrence, not an instruction inside that occurrence.
 * Skipping the draw step differs from replacing one draw (CR614.10). */
export type SkippableTime =
  | { kind: "turn" }
  | { kind: "phase"; phase: "main" | "combat" }
  | { kind: "step"; step: "untap" | "upkeep" | "draw" };

export type Effect = EffectBase &
  (
    /** Create a replacement for upcoming occurrences (CR614.10). A skipped
     * occurrence satisfies only one next-occurrence replacement; effects
     * waiting for other occurrences remain pending. Cannot skip an already
     * started occurrence. A duration, if present, limits that pending effect.
     * `during` scopes all matching occurrences of one specified future turn,
     * not a duration ending when that turn begins. */
    | { effect: "skip-time"; who: PlayerRef; what: SkippableTime;
        occurrences: { kind: "next"; count: Amount } | { kind: "all" };
        during?: { turn: "next"; whose: PlayerRef } }
    /** CR500.7: insert extra turns one at a time after the anchor; APNAP
     * determines insertion for multiple players, most recently added first. */
    | { effect: "add-turns"; who: PlayerRef; count: Amount; after: { turn: "current" } }
    /** CR500.8: the printed groups form an ordered sequence, inserted after
     * the specified phase. A main/combat qualifier is an anchor constraint,
     * not permission to create a phase after an unrelated current phase. */
    | { effect: "add-phases"; after:
          | { phase: "current"; kind?: "main" | "combat" }
          | { phase: "ordinal"; kind: "main" | "combat"; ordinal: number; turn: "current" };
        sequence: { phase: "main" | "combat"; count: Amount }[] }
    /** Intrinsic special procedures of CR724.1/2. Both exile the stack and
     * terminate normal resolution; ending combat is ineffective outside
     * combat, whereas ending the turn proceeds to a cleanup step. */
    | { effect: "end-turn" | "end-combat" }
    /** Explicit quantified instructions, not a scalar quantity multiplier.
     * CR 608.2e/f, 101.4: process each instruction across all members before
     * the next instruction, with APNAP choices and simultaneous actions
     * where possible. Targets remain declarations of the original ability
     * (601.2c, 115.3), with their printed lexical selection identities.
     * Member anaphors, membership timing and aggregated results require a
     * dedicated binding scope; they are not prior global event results. */
    | { effect: "for-each"; each: ObjectRef | PlayerRef; effects: Sentence[]; processing: "instruction-wise" }
    | { effect: "roll-dice"; who: PlayerRef; die: DieSpec; count: Amount;
        adjustments?: { operation: "add" | "subtract"; appliesTo: "single-result"; amount: Amount }[];
        /** Ignored dice never produce roll events or usable results (706.6).
         * The roller chooses between tied lowest results when necessary. */
        ignore?: { ignore: "lowest"; count: 1 } | { ignore: "all-but-highest" };
        table?: DiceResultTable }
    /** CR 706.3c: a fresh roll using the enclosing table's original die kind,
     * count and applicable modifiers, followed by that table again. This is
     * distinct from rerolling a natural result as a modifier under 706.2b. */
    | { effect: "roll-again"; who: PlayerRef; of: { roll: "enclosing-table" } }
    /** CR 705: `who` flips `count` coins (a flip "until you lose" repeats). A
     * called flip is won or lost by the flipper (705.2); a result-only flip
     * comes up heads or tails. Every outcome clause printed after the flip
     * belongs here (never a separate sentence), and all outcomes of one flip
     * use the same vocabulary: win/lose or heads/tails. */
    | { effect: "flip-coin"; who: PlayerRef; count: Amount; until?: "lose"; outcomes?: CoinFlipOutcome[] }
    /** The game, not a player, selects at random among the matching objects or
     * players; nobody chooses and nothing is targeted. */
    | { effect: "choose-random"; what: ObjectRef | PlayerRef }
    | { effect: "draw"; who: PlayerRef; amount: Amount }
    | { effect: "get-counters"; who: PlayerRef; counter: CounterSpec; count: Amount }
    | { effect: "get-emblem"; who: PlayerRef; abilities: GainedAbility[] }
    /** Target choices are announced under CR 601.2c, not during resolution. */
    | { effect: "declare-target"; what: ObjectRef | PlayerRef }
    | { effect: "choose-object"; who: PlayerRef; what: ObjectRef | PlayerRef }
    | ({ effect: "choose-value"; who: PlayerRef } & ValueChoice)
    /** CR 701.27–28: turn the existing double-faced permanent over, not a
     * zone change and not the face-up/face-down game action. */
    | { effect: "transform" | "convert"; what: ObjectRef; who?: PlayerRef }
    /** CR 710.4: flipping a flip-card permanent is a one-way status change,
     * distinct from transforming and from turning face up/down. */
    | { effect: "flip"; what: ObjectRef }
    /** CR 701.40a/701.58a: `who` turns each card face down and puts it onto
     * the battlefield face down. A cloaked permanent also has ward {2}
     * (701.58a). Several cards are processed one at a time (701.40e/701.58e);
     * `what` names the cards, not the resulting permanents. */
    | { effect: "manifest" | "cloak"; who: PlayerRef; what: ObjectRef }
    /** CR 701.62a: look at the top two cards of the library, manifest one and
     * put the other into the graveyard; `times` repeats the whole procedure. */
    | { effect: "manifest-dread"; who: PlayerRef; times?: Amount }
    /** CR 708.2a/708.8: an object turned face down becomes a 2/2 face-down
     * creature unless the instruction lists other characteristics; turning
     * face up restores its normal characteristics. Not transform (701.27b). */
    | { effect: "turn-face"; what: ObjectRef; face: "up" | "down"; who?: PlayerRef;
        characteristics?: FaceDownCharacteristics }
    /** CR 701.35a: until the next turn of the controller of the detaining
     * spell or ability, the permanent can't attack or block and its
     * activated abilities can't be activated. */
    | { effect: "detain"; what: ObjectRef }
    /** CR 701.60a: the creature becomes suspected; "no longer suspected" ends
     * that designation (suspected is a designation, 701.60b). */
    | { effect: "suspect" | "unsuspect"; what: ObjectRef }
    /** CR 731.1: the game gains the day or night designation. */
    | { effect: "set-day-night"; to: "day" | "night" }
    /** CR 701.3: attachment legality is checked when the action occurs;
     * this instruction does not itself change control of either object. */
    | { effect: "attach"; what: ObjectRef; to: ObjectRef | PlayerRef; who?: PlayerRef }
    | { effect: "unattach"; what: ObjectRef; from?: ObjectRef | PlayerRef; who?: PlayerRef }
    /** CR 702.26: phasing changes status without leaving/entering a zone. */
    | { effect: "phase-out" | "phase-in"; what: ObjectRef }
    /** targets describes selections in the body; a target named in the
     * trigger event is instead selected by the creating spell/ability. */
    | { effect: "create-delayed-trigger"; trigger: Trigger; effects: Sentence[]; occurrence: "next" | "each"; nextOnly?: true; targets: "enclosing-ability" | "delayed-ability"; triggerTargets?: "enclosing-ability"; condition?: Condition }
    | { effect: "create-reflexive-trigger"; event: { event: "preceding-action"; who: PlayerRef; performed: boolean }; effects: Sentence[] }
    | { effect: "create-replacement"; replacement: ReplacementInstruction }
    /** CR 614.15 / 616.1a: modifies this spell/ability's own instruction and
     * applies before other replacements. Alternative kicker targets are
     * announced from the applicable branch, not from both (601.2c, 702.33g). */
    | { effect: "self-replacement"; original: Effect; condition: Condition; instead: Effect[];
        targetPolicy: "retain-original" | "alternative-by-kicker";
        /** Declining an optional replacement performs the original effect;
         * this is not an optional action after replacing the original. */
        replacementChoice?: { who: PlayerRef; declined: "original" };
        /** CR 614.15: the replacement was printed as its own paragraph after
         * the replaced ability; this is that paragraph's ability word, if
         * any (207.2c: no rules meaning). The block spans both paragraphs. */
        abilityWord?: string }
    | { effect: "optional-group"; who: PlayerRef; steps: Effect[] }
    /** CR 608.2d: exactly one listed instruction is performed, chosen by who
     * while the effect is applied. It is not a mode announced with the spell
     * or ability (700.2), and an illegal or impossible option can't be
     * chosen. A subject, object, or target shared by the printed alternatives
     * is the same reference (and target selection) in every option. */
    | { effect: "choice"; who: PlayerRef; options: Effect[] }
    | { effect: "simultaneous"; steps: Effect[] }
    | { effect: "pay-cost"; who: PlayerRef; costs: Cost[] }
    | { effect: "play-card"; instruction: CardPlayInstruction }
    | { effect: "grant-play-permission"; permission: CardPlayInstruction; duration: Duration; durationScope?: "each-permitted-card" }
    | { effect: "grant-look-permission"; permission: LookPermission; duration: Duration }
    /** CR 305.2: for the duration, raises the number of lands the player can
     * play in a turn. "May" describes the later land plays; creating this
     * continuous effect is mandatory (611.2a). See LandPlayAllowance. */
    | { effect: "additional-land-plays"; allowance: LandPlayAllowance; duration: Duration }
    /** `times` is "twice" / "N times": the action happens that many times in
     * sequence, as separate events (not one scaled event). */
    | { effect: "keyword-action"; action: "proliferate" | "populate" | "investigate"; who: PlayerRef; times?: Amount }
    /** Designations and player-level keyword actions with no further
     * parameters: CR 725.2 (become the monarch), 726.2 (take the initiative),
     * 701.54a (the Ring tempts), 701.48a, 701.61a, 701.56a, 701.51b. */
    | { effect: "become-monarch" | "take-initiative" | "ring-tempts" | "learn" | "forage" | "time-travel" | "open-attraction"; who: PlayerRef }
    /** CR 701.49a-c: "venture into the dungeon" (not the quality variants). */
    | { effect: "venture"; who: PlayerRef }
    /** CR 701.53a: create an Incubator token with `amount` +1/+1 counters;
     * `times` repeats it ("incubate 2 that many times"). */
    | { effect: "incubate"; who: PlayerRef; amount: Amount; times?: Amount }
    /** CR 701.66a: the implied target ("target land you control") is part of
     * the instruction, so it is carried here as `what`. */
    | { effect: "earthbend"; who: PlayerRef; amount: Amount; what: ObjectRef }
    /** CR 701.65a. */
    | { effect: "airbend"; who: PlayerRef; what: ObjectRef }
    /** CR 701.15a: until the next turn of the controller of the spell or
     * ability that goaded it. The goading player is the controller. */
    | { effect: "goad"; what: ObjectRef }
    /** CR 701.63a: the permanent's controller makes a Spirit or adds counters. */
    | { effect: "endure"; what: ObjectRef; amount: Amount }
    /** CR 701.71a ("empower Jace N"). */
    | { effect: "empower-jace"; who: PlayerRef; amount: Amount }
    /** CR 701.30a-b: `with` is the opponent ("an opponent", "defending player"). */
    | { effect: "clash"; who: PlayerRef; with: PlayerRef }
    /** CR 701.38a: every player votes, starting with `starting` when stated.
     * `secret`: votes are cast secretly and then revealed. */
    | { effect: "vote"; starting?: PlayerRef; secret?: true; choices: VoteChoices }
    /** CR 111.10j-r: a Role token (Aura) created attached to `attachedTo`
     * (CR 303.7a); the role's abilities are those defined by the CR. */
    | { effect: "create-role"; role: RoleName; count: Amount; attachedTo: ObjectRef; who?: PlayerRef }
    /** CR 701.70a: draw a card, then discard a card; if a nonland card was
     * discarded this way, create a 1/1 white Human Soldier creature token. */
    | { effect: "keyword-action"; action: "recruit"; who: PlayerRef }
    /** CR 701.68: who puts amount -1/-1 counters on one creature they
     * control, chosen as the action is performed. A player who cannot do so
     * cannot choose to blight (701.68b). */
    | { effect: "blight"; who: PlayerRef; amount: Amount }
    | { effect: "discover"; who: PlayerRef; amount: Amount }
    | { effect: "amass"; subtype: string; amount: Amount }
    | { effect: "adapt" | "monstrosity" | "bolster" | "support"; amount: Amount }
    /** `times`: "explores X times" / "explores, then it explores again". */
    | { effect: "explore"; what: ObjectRef; times?: Amount }
    /** `amount`: connive N (CR 701.50d). */
    | { effect: "connive"; what: ObjectRef; amount?: Amount }
    | { effect: "damage"; source: ObjectRef; amount: Amount; to: DamageTarget }
    /** A no-regeneration rider modifies this destruction (CR 608.2c),
     * rather than resolving after the permanent has already been destroyed. */
    | { effect: "destroy"; what: ObjectRef; regenerationForbidden?: true }
    /** CR 406.3: "face down" is information-bearing; a face-down exiled card
     * has no characteristics and cannot be examined except as instructed.
     * `counters` are on the card as it arrives in exile (CR 702.62a's
     * "exile it with N time counters on it"), not a later put-counters step. */
    | { effect: "exile"; what: ObjectRef; from?: ZoneRef; who?: PlayerRef; faceDown?: true; counters?: { counter: CounterSpec; count: Amount }[] }
    /** CR 610.3: one simultaneous exile move linked to a second one-shot
     * return, not a delayed triggered ability or a duration on ordinary exile.
     * If the return event already occurred within the CR 610.3a/b window,
     * nothing moves. Returned battlefield objects default to owner control.
     * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
    | { effect: "exile-until"; what: ObjectRef[]; from?: ZoneRef; who?: PlayerRef;
        until: { event: "leaves-battlefield"; what: ObjectRef };
        return: { to: "previous-zone"; battlefieldController: "owner" } }
    | { effect: "counter"; what: ObjectRef; countered?: CounteredSpellDestination }
    | { effect: "move-zone"; what: ObjectRef; to: ZoneRef; from?: ZoneRef; tapped?: boolean; controller?: "you" | "owner"; order?: "any" | "random"; who?: PlayerRef; entry?: BattlefieldEntryModifiers }
    /** `their` keeps the printed pronoun for the hand of the acting or just
     * mentioned player; CR 400.3 still sends each card to its owner's hand. */
    | { effect: "return-to-hand"; what: ObjectRef; to: "your" | "owner" | "their"; from?: ZoneRef; who?: PlayerRef }
    | { effect: "create-token"; count: Amount; token: TokenSpec; who?: PlayerRef }
    | { effect: "pump"; what: ObjectRef; power: SignedAmount; toughness: SignedAmount }
    /** CR 701.10a-c / 701.11: doubling (or tripling) power and/or toughness is
     * a layer 7c modifier (+X/+0, +0/+X) computed from the value as the
     * effect begins to apply. It is not a set-value effect. `times` repeats
     * the doubling, each time from the value then current. */
    | { effect: "multiply-pt"; what: ObjectRef; stat: "power" | "toughness" | "both"; factor: "double" | "triple"; times?: Amount }
    /** CR 613.4d (layer 7d), 701.12g exclusion: a switch is a continuous
     * effect with a duration, not an exchange of two numerical values. */
    | { effect: "switch-pt"; what: ObjectRef }
    | { effect: "set-pt"; what: ObjectRef; power: Amount; toughness: Amount }
    | { effect: "lose-abilities"; what: ObjectRef; removal: AbilityRemoval }
    /** CR 113.11: the objects lose each listed ability and no effect or
     * keyword counter can add it while this effect lasts. */
    | { effect: "cant-have-abilities"; what: ObjectRef; abilities: KeywordInstance[] }
    /** The printed pronoun "they" alone does not establish whether its
     * recipient is an object set or a player (CR 109.1, 113.1). Keep that
     * distinction unresolved until antecedent binding, rather than treating
     * returned creatures as players because the player grammar runs first. */
    | { effect: "gain-abilities"; what: ObjectRef | { recipient: "they" }; abilities: GainedAbility[] }
    | { effect: "player-gain-abilities"; who: PlayerRef; abilities: GainedAbility[] }
    | { effect: "gain-life" | "lose-life"; who: PlayerRef; amount: Amount }
    /** CR 701.10d: gain or lose life so the new total is twice the current. */
    | { effect: "double-life-total"; who: PlayerRef }
    /** CR 701.10f: add as much of each type of mana as is already in the pool. */
    | { effect: "double-mana-pool"; who: PlayerRef }
    /** An explicit instruction to win/lose, subject to game rules and
     * replacement/prohibition effects (CR 104.2b, 104.3e), not life loss. */
    | { effect: "win-game" | "lose-game"; who: PlayerRef }
    | { effect: "tap" | "untap"; what: ObjectRef; who?: PlayerRef }
    /** CR 506.4: the permanent stops being an attacking, blocking, blocked
     * and/or unblocked creature (or stops being attacked). It does not
     * change zones or tapped status, and is ineffective outside combat. */
    | { effect: "remove-from-combat"; what: ObjectRef }
    | { effect: "untap-step-suppression"; what: ObjectRef; step: UntapStepWindow }
    | { effect: "sacrifice"; who: PlayerRef; what: ObjectRef; chosenBy?: PlayerRef }
    | { effect: "discard"; who: PlayerRef; what: ObjectRef | "hand"; random?: boolean }
    | { effect: "mill"; who: PlayerRef; amount: Amount }
    | { effect: "scry" | "surveil"; amount: Amount; who?: PlayerRef }
    | { effect: "put-counters"; counter: CounterSpec; count: Amount; on: ObjectRef; who?: PlayerRef }
    /** counter "any": the removing player chooses among the counters present,
     * of any kinds. upTo permits any smaller number, including zero. */
    | { effect: "remove-counters"; counter: CounterSpec | "any"; count: Amount | "all"; from: ObjectRef; upTo?: true }
    /** CR 701.10e: give the player or permanent as many counters of that kind
     * as it already has. "each-kind" doubles every kind separately. */
    | { effect: "double-counters"; on: ObjectRef; counter: CounterSpec | "each-kind" }
    /** CR 122.5: remove counters from `from`, then put them onto `to`. If either
     * half is impossible nothing moves. `"any"` is any kind of counter; with a
     * count of 1 the kind is chosen on resolution, with "all" every kind moves. */
    | { effect: "move-counters"; counter: CounterSpec | "any"; count: Amount | "all"; from: ObjectRef; to: ObjectRef }
    /** CR 601.2d / 608.2d: the division is announced with the targets (or, for
     * an untargeted group, chosen on resolution); each recipient gets at least one. */
    | { effect: "distribute-counters"; counter: CounterSpec; count: Amount; among: ObjectRef }
    | { effect: "add-mana"; who: PlayerRef; mana: ManaProduction; restriction?: ManaSpendingRestriction; riders?: ManaSpendingRider[] }
    | { effect: "search"; who: PlayerRef; zones: SearchZones; for: ObjectRef }
    | { effect: "shuffle"; who: PlayerRef; library: ZoneRef }
    /** One grouped move followed by a library shuffle even when no specified
     * objects move (CR 701.24c–d), not a shuffle conditional on moving a card. */
    | { effect: "shuffle-into-library"; who: PlayerRef; what: ObjectRef[]; library: ZoneRef }
    | { effect: "gain-control"; who: PlayerRef; what: ObjectRef }
    /** CR 701.12a-b: all-or-nothing and simultaneous; the operands are either
     * two refs or one selection of exactly two objects. Same controller: no-op. */
    | { effect: "exchange-control"; what: ObjectRef[] }
    /** CR 701.12c: each player gains or loses life to equal the other's total. */
    | { effect: "exchange-life-totals"; between: [PlayerRef, PlayerRef] | ({ players: "two-target" } & TargetSelection) }
    /** CR 701.12g: each value becomes the other's previous value. A life total
     * is adjusted by gain/loss; power/toughness by a layer 7b setting effect. */
    | { effect: "exchange-values"; a: ExchangeValue; b: ExchangeValue }
    /** CR 701.12d/f: exchange the contents of two zones of one player. */
    | { effect: "exchange-zones"; a: ZoneRef; b: ZoneRef }
    | { effect: "reveal"; who: PlayerRef; what: ObjectRef }
    /** Process successive cards from the top, including the last match.
     * Reveal keeps those cards in the library (701.20b); exile moves each
     * card (701.13). Stop at the requested matching-card count or exhaustion
     * (609.3); zero requires no cards. Quantified actors run separate scans,
     * and the revealed/exiled pool is distinct from its matching subset. */
    | { effect: "scan-library"; who: PlayerRef; operation: "reveal" | "exile";
        library: ZoneRef; until: { count: Amount; matching: ObjectFilter } }
    | { effect: "look-at"; who: PlayerRef; what: ObjectRef | LookTop }
    | { effect: "fight"; a: ObjectRef; b: ObjectRef }
    | { effect: "modal"; count: ModalCount; options: ModalOption[]; allowRepeat?: true;
        /** The modes are selected by the game at random, not by a player. */
        random?: true;
        keyword?: "spree" | "tiered";
        /** The same modal ability on this source remembers chosen modes;
         * changing zones creates a new source object (CR 400.7). */
        unchosen?: { period: "turn" | "source-object" } }
    /** CR 707.10: put count copies of EACH selected spell/activated/triggered
     * ability onto the stack under who's control, without casting/activating.
     * Retain original decisions and, for abilities, the original source. */
    | { effect: "copy-stack-object"; what: ObjectRef; who: PlayerRef; count: Amount;
        /** CR 707.10c: choose for each copy before placing it on the stack;
         * declining preserves copied targets. Does not change modes/division. */
        retarget?: { who: PlayerRef; optional?: true } }
    /** CR 115.7d: preserve modes/division while optionally rechoosing targets
     * of every referenced object; the printed referent is never implicit. */
    | { effect: "choose-new-targets"; what: ObjectRef; who: PlayerRef;
        timing: "resolution" | "copy-creation" }
    | { effect: "regenerate"; what: ObjectRef }
    | { effect: "prevent-damage"; prevention: DamagePrevention }
    | { effect: "prohibit"; prohibition: GameProhibition }
    | { effect: "cast-limit"; limit: CastLimit }
    /** A payment adjustment created on resolution for its stated duration
     * (CR 601.2f, 611.2a); it never changes a mana cost or mana value. */
    | ({ effect: "cost-modification"; scope: CostModificationScope } & CostModification)
    | { effect: "become"; what: ObjectRef; spec: BecomeSpec }
    /** One continuous effect across layers 4–7 (CR 613.6). The affected
     * objects and direct values are fixed when it begins (611.2c/d);
     * calculations inside granted abilities remain those abilities' own
     * calculations. Ordinary grants cannot create CDAs (604.3a). */
    | { effect: "modify-characteristics"; what: ObjectRef;
        modifiers: ResolvingCharacteristicModifier[]; snapshot: "effect-created";
        /** CR 611.2e: stated as "is/isn't [characteristic]" or "has
         * [characteristic]" instead of "becomes". If this resolution also
         * puts the affected nontoken permanent onto the battlefield, the
         * effect applies simultaneously with that entry rather than once
         * the permanent is there. */
        statedAs?: "is" | "has" }
    | { effect: "cant"; what: ObjectRef; action: CantAction }
    /** CR 509.1h, 509.3c-d: an effect makes the attacker a blocked creature
     * without any creature blocking it. */
    | { effect: "become-blocked"; what: ObjectRef }
    | { effect: "combat-rule"; what: ObjectRef; rule: CombatRule }
  );

/** CR 608.2c, 701.6a: later text modifies the counter event itself. A spell
 * actually countered by that instruction is put `to` instead of into its
 * owner's graveyard; a spell that isn't countered doesn't move. `only`
 * limits this to matching countered spells (Desertion); `controller` is
 * printed for a battlefield destination. Never a later zone-change effect. */
export interface CounteredSpellDestination { to: ZoneRef; only?: ObjectFilter; controller?: "you" }

export interface LookTop { top: Amount; of: ZoneRef }

export interface CoinFlipOutcome { result: "win" | "lose" | "heads" | "tails"; effects: Sentence[] }

export type ExchangeValue =
  | { value: "life-total"; of: PlayerRef }
  | { value: "power" | "toughness"; of: ObjectRef };

/** Modifies the same battlefield-entry event. Counters and attachments are
 * already present as it enters; these are never later put-counter/attach
 * instructions. CR 122.6, 303.4f–h, 508.4.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export interface BattlefieldEntryModifiers {
  counters?: { counter: CounterSpec; count: Amount; additional?: true }[];
  /** CR 508.4 chooses a legal defending player, planeswalker, or battle for
   * each entering creature unless this instruction names its defender. */
  attacking?: { choice: "entering-controller" } | { defender: PlayerRef | ObjectRef };
  attachedTo?: PlayerRef | ObjectRef;
  /** CR 712.14a: a double-faced card put onto the battlefield "transformed"
   * or "converted" enters with its back face up. */
  face?: "transformed" | "converted";
  /** CR 708.2–708.3: put onto the battlefield face down, turned face down
   * before it enters. Without listed characteristics it is a 2/2 face-down
   * creature (708.2a). */
  faceDown?: { characteristics?: FaceDownCharacteristics };
}

/** The characteristics an effect lists for a face-down object (CR 708.2);
 * a face-down object has no other characteristics. */
export interface FaceDownCharacteristics {
  power?: Amount;
  toughness?: Amount;
  types: string[];
  subtypes: string[];
}

export type ModalCount =
  | { exactly: number }
  | { min: number; max: number }
  | { variable: "x" }
  | { atLeast: number } // "one or more"
  /** CR 700.2i: sum of selected mode weights, not the number of modes. */
  | { weightBudget: number };

export interface ModalOption {
  effects: Sentence[];
  /** Flavor label, not a condition or an extra instruction. */
  label?: string;
  /** CR 700.2h / 702.172 / 702.183: payable while casting if chosen. */
  additionalCost?: Cost[];
  /** Pawprint symbols preceding this particular mode (CR 700.2i). */
  weight?: number;
}

export type DamageTarget =
  | ({ to: "any-target"; random?: true } & TargetSelection)
  | { to: "object"; ref: ObjectRef }
  | { to: "player"; ref: PlayerRef }
  /** CR 508.1b, 506.4c: whatever the referenced creature is attacking as the
   * damage is dealt, if it is one of the printed kinds; otherwise there is
   * no recipient. Neither a target nor a choice. */
  | { to: "attacked"; by: ObjectRef; kinds: ("player" | "planeswalker" | "battle")[] }
  | { to: "each"; refs: (ObjectRef | PlayerRef)[] }
  /** CR 601.2d: "divided as you choose" is announced with the targets; "evenly,
   * rounded down" is split equally by the game with no choice. */
  | { to: "divided"; among: ObjectRef; evenly?: "round-down" };

/** Recurring windows are checked whenever the prohibited action is proposed;
 * they are not an expiry such as "this combat" (CR 611.2c, 611.3a). */
export type ProhibitionWindow = { during: "combat" } | { during: "turn"; whose: PlayerRef }
  /** One identified turn: the next turn of the stated player after the
   * effect is created. "That player" is each prohibited player in turn. */
  | { during: "next-turn"; whose: PlayerRef };

/** "Their hands" is evaluated independently for each acting player, never
 * the union of every affected player's hand. Missing owner permits any owner. */
export interface ProhibitionSource {
  relation: "in" | "outside";
  zones: (Omit<ZoneRef, "owner"> & { owner?: ZoneRef["owner"] | "acting-player" })[];
}

/** A rule prohibition, distinct from removing abilities or countering an
 * existing spell. These match proposed actions dynamically, including rules
 * created during resolution (CR 101.2, 601.3, 602.1c, 611.2c, 615.12).
 * Ability-source queries are not choices of a source. Absent activation who
 * means any player. Playing a card includes land play and casting (701.18b).
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export type GameProhibition = { when?: ProhibitionWindow } & (
  | { prohibition: "cast" | "play"; who: PlayerRef; what: ObjectRef; from?: ProhibitionSource }
  | { prohibition: "activate"; who?: PlayerRef; abilities: ObjectRef }
  | { prohibition: "prevent-damage"; kind: "all" | "combat" | "noncombat"; by?: ObjectRef; to?: (ObjectRef | PlayerRef)[] }
);

/** All matching spells already cast in the current turn count, even before
 * this rule began. Each player's total is independent; copies not cast do
 * not count (CR 601.2i, 707.10, 611.3). */
export interface CastLimit {
  who: PlayerRef;
  what: ObjectFilter;
  maximum: Amount;
  period: "turn";
  count: "each-player-all-casts";
}

/** CR 609.7a includes spells and referenced former objects, not just
 * battlefield permanents. Choice occurs on resolution, never as a target;
 * source properties are rechecked when damage would occur (609.7b). */
export interface DamageSourceChoice {
  choice: "damage-source";
  who: PlayerRef;
  filter: ObjectFilter;
  when: "effect-created";
  universe: "damage-sources";
  recheck: "when-damage-would-be-dealt";
}
export type PreventionQuantity =
  | { prevent: "all" }
  /** up-to is mandatory min(incoming damage, amount), not a choice of any
   * smaller value. Optional application is represented by optional.who. */
  | { prevent: "up-to" | "all-but"; amount: Amount }
  | { prevent: "half"; round: "up" | "down" };
/** CR 615.7 cumulative budgets, 615.8 next events, and 615.10 recurring
 * per-event prevention are different shields. The printed event controls
 * simultaneous grouping (700.1): "a source" matches each source separately;
 * unspecified incoming damage may combine sources (Phantom creatures).
 * Per-recipient budgets take their recipient snapshot when the effect is
 * created (615.11); a combined recipient list otherwise shares one budget.
 * Untargeted source/recipient filters match future events; they do not
 * choose one object when the effect is created. */
export interface DamagePrevention {
  eventScope: { sources: "each" | "simultaneous"; recipients: "each" | "simultaneous" };
  shield:
    | { shield: "damage-budget"; amount: Amount; allocation?: "per-recipient" }
    | { shield: "events"; occurrence: "each" | "next"; prevent: PreventionQuantity };
  kind: "all" | "combat" | "noncombat";
  by?: ObjectRef | DamageSourceChoice;
  to?: (ObjectRef | PlayerRef)[];
  /** Damage either dealt to or dealt by this object. */
  involving?: ObjectRef;
  /** Restricts the incoming damage event, before this shield applies. */
  incomingAmount?: Comparison;
  condition?: Condition;
  optional?: { who: PlayerRef };
  /** Resolving continuous effects fix X at creation (611.2d). A static
   * rule's reduction is evaluated when a matching event occurs (615.10). */
  bindings?: { variable: "x"; value: Amount; when: "effect-created" | "event-application" }[];
  /** Execute immediately after the prevention attempt (615.5), even when
   * damage cannot be prevented (615.12). A damage-prevented condition tests
   * actual prevention, and quantities distinguish incoming from prevented. */
  after?: Sentence[];
}

export type GainedAbility =
  | { gained: "keyword"; keyword: KeywordInstance; protectionAttachmentException?: ProtectionAttachmentException;
      /** The keyword's cost parameter, printed as its own sentence ("The
       * flashback cost is equal to its mana cost") instead of after the name. */
      costDefinition?: GrantedKeywordCost }
  /** Exactly `count` of the listed keywords, chosen by `who` while the
   * granting instruction is applied (CR 608.2d); the choice is not a target
   * and the unchosen keywords are not granted. One choice per instruction. */
  | { gained: "keyword-choice"; who: PlayerRef; count: Amount; options: KeywordInstance[]; when: "granting-effect-applies" }
  /** Every ability of the listed kinds that the referenced objects have, as
   * abilities of the recipient (CR 113.10, 201.5b: a gained ability's name
   * references mean the recipient). A static grant reevaluates the referenced
   * objects and their abilities (611.3a); a resolving grant determines them
   * once (608.2h). `except` removes that class from the granted set. */
  | { gained: "abilities-of"; kinds: ("activated" | "triggered" | "loyalty")[]; of: ObjectRef; except?: "mana-abilities" }
  | { gained: "quoted"; ability: Ability };

/** A granted keyword's cost computed for each object that has the keyword:
 * that object's own mana cost (CR 202.1), which alternative costs and cost
 * changes never alter (118.9c). `reducedBy` is the printed "reduced by"
 * mana, applied to this cost as a reduction (118.7); `additional` are the
 * further printed components after "plus". An object without a mana cost
 * yields an unpayable cost (118.6), not a free one. */
export interface GrantedKeywordCost {
  manaCostOf: "each-recipient";
  reducedBy?: string[];
  additional?: Cost[];
}

/** CR 702.16n/p: an exception belongs to this particular protection grant.
 * It affects only attachment state-based actions, not targeting or permission
 * for new attachments. Other instances of protection still apply normally.
 * References here belong to the granting effect's scope, not the permanent
 * that receives the keyword. https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export type ProtectionAttachmentException =
  | { exception: "source-aura"; aura: ObjectRef }
  | { exception: "auras" }
  | { exception: "already-attached"; filter: ObjectFilter; to: ObjectRef; snapshot: "when-protection-effect-begins" };

/** Removing abilities (layer 6) differs from setting base P/T (layer 7b).
 * CR 613.1f, 613.4b; choosing one keyword is not removing every option. */
export type AbilityRemoval =
  | { removal: "all" }
  /** "loses all other abilities": every ability except those this same
   * continuous effect gives the object; both parts apply in layer 6 with one
   * timestamp (CR 613.1f, 613.6). */
  | { removal: "all-other"; than: "abilities-granted-by-this-effect" }
  | { removal: "listed"; abilities: GainedAbility[] }
  | { removal: "choice"; who: PlayerRef; count: Amount; options: KeywordInstance[] };

/** What a vote is for (CR 701.38b): words connected to different effects,
 * objects, or players. */
export type VoteChoices =
  | { choices: "named"; names: string[] }
  | { choices: "objects"; filter: ObjectFilter }
  | { choices: "players"; other?: true };

/** CR 111.10j-r: the Role tokens defined by the Comprehensive Rules. */
export type RoleName = "Cursed" | "Monster" | "Royal" | "Sorcerer" | "Virtuous" | "Wicked" | "Young Hero";

export type PredefinedTokenName = "Treasure" | "Food" | "Gold" | "Walker" | "Shard" | "Clue" | "Blood" | "Powerstone" | "Incubator" | "Map" | "Junk" | "Lander" | "Mutagen" | "Vibranium" | "Heartwood";

/** A predefined token references its complete CR 111.10 definition. It is
 * distinct from an explicitly specified token sharing that subtype. */
export type TokenSpec = SpecifiedTokenSpec | {
  predefined: PredefinedTokenName;
  tapped?: boolean;
  attacking?: boolean;
} | {
  copyOf: ObjectRef;
  modifications?: CopyModification[];
  tapped?: boolean;
  attacking?: boolean;
};

/** Exceptions become part of a token's copiable values (CR 707.9). */
export type CopyModification =
  | { modification: "not-legendary" }
  | { modification: "characteristics"; spec: BecomeSpec }
  | { modification: "abilities"; abilities: GainedAbility[] };

export interface SpecifiedTokenSpec {
  name?: string;
  power?: Amount;
  toughness?: Amount;
  colors: Color[];
  supertypes: string[];
  types: string[];
  subtypes: string[];
  keywords: KeywordInstance[];
  abilities?: GainedAbility[];
  tapped?: boolean;
  attacking?: boolean;
}

export interface BecomeSpec {
  copyOf?: ObjectRef;
  types?: string[];
  subtypes?: string[];
  colors?: Color[];
  power?: Amount;
  toughness?: Amount;
  inAddition?: "types" | "colors";
}

export type ResolvingCharacteristicModifier =
  | { modifier: "types"; change: TypeChange }
  | { modifier: "colors"; operation: "set" | "add"; colors: Color[] | ChosenValueRef<"color"> | FreshCharacteristicChoice<"color"> }
  /** CR 613.4b: an unstated base value keeps its current definition. */
  | ({ modifier: "base-pt";
      /** A bare possessive in a direct value denotes this affected object,
       * while an explicit source reference still denotes the source. */
      valueScope?: "each-affected-object" } & (
      | { power: Amount; toughness?: Amount }
      | { power?: Amount; toughness: Amount }))
  | { modifier: "abilities"; abilities: GainedAbility[] };

/** "phase-in"/"phase-out" (CR 702.26), "transform" (CR 701.27; also blocks
 * converting, 701.28f), "be-turned-face-up" (CR 708) and "become-suspected"
 * (CR 701.60) are status-change prohibitions. */
export type CantAction = "attack" | "block" | "attack-or-block" | "be-blocked" | "untap" | "be-countered" | "be-regenerated"
  | "phase-in" | "phase-out" | "transform" | "be-turned-face-up" | "become-suspected";

export type CombatDefender =
  | { defender: "player"; who: PlayerRef }
  | { defender: "object"; what: ObjectRef }
  | { defender: "player-other-than"; who: PlayerRef };

/** CR 508.1c–d, 509.1a–c: restrictions, requirements, and capacities have
 * different effects on combat legality. Conditions are continuously checked,
 * not one-time tests when an effect that creates the rule resolves. */
export type CombatRule = { condition?: Condition; conditionKind?: "if" | "unless" } & (
  | { rule: "blocker-filter"; mode: "exclude" | "only"; blockers: ObjectRef }
  /** Constrains only NONEMPTY blocking assignments to this attacker. Even a
   * minimum allowed number never requires any creature to block it. */
  | { rule: "blocker-count"; mode: "exclude" | "only"; comparison: Comparison }
  | { rule: "blocking-filter"; mode: "exclude" | "only"; attackers: ObjectRef }
  | { rule: "block-capacity"; capacity: { additional: Amount } | { maximum: Amount | "unlimited" } }
  | { rule: "attack-requirement"; frequency: "each-combat" | "each-turn"; defender?: CombatDefender }
  /** CR 509.1c: each affected creature blocks if able, at every declare
   * blockers step while the rule exists. With attackers, each must block
   * that attacker; "all creatures able to block X do so" is this rule on
   * every creature. Absent attackers, any legal block satisfies it. */
  | { rule: "block-requirement"; attackers?: ObjectRef }
  /** CR 509.1c: the affected attacker must be blocked if able. This is one
   * requirement on the defending player's declaration, satisfied by a single
   * qualifying blocker, not a requirement on each potential blocker.
   * blockers limits which creatures satisfy it; count is the printed number
   * of creatures that must block it if able. */
  | { rule: "blocked-requirement"; blockers?: ObjectRef; count?: Comparison }
  | { rule: "attack-restriction"; defenders: CombatDefender[] }
  /** CR 506.5: alone means the only creature declared as an attacker or as
   * a blocker in that declaration. exclude forbids being declared alone;
   * only forbids being declared together with any other creature. */
  | { rule: "alone-restriction"; action: "attack" | "block"; mode: "exclude" | "only" }
  | { rule: "all-of"; rules: CombatRule[] }
);

export type ManaProduction =
  | { mana: "fixed"; symbols: string[] }                    // Add {G}{G}
  | { mana: "choice"; options: ManaProduction[] }           // Add {R} or one mana of the chosen color
  | { mana: "chosen-color"; amount: Amount; choice: ChosenValueRef<"color"> }
  | { mana: "any-color"; amount: Amount }                   // Add two mana of any color
  | { mana: "any-type"; amount: Amount }
  | { mana: "any-one-color"; amount: Amount }               // Add three mana of any one color
  | { mana: "commander-color"; amount: Amount; whose: PlayerRef }
  | { mana: "combination"; amount: Amount; options: "colors" | "types" | string[] }
  | { mana: "scaled-symbol"; amount: Amount; symbol: string }
  | { mana: "could-produce" | "produced"; amount: Amount; what: ObjectRef; colorOnly: boolean };

/** CR 106.6: restrictions belong to produced mana, not the producer's future
 * spells generally. Allowed payment purposes are alternatives. */
export interface ManaSpendingRestriction {
  restriction: "only" | "not";
  uses: ManaSpendingUse[];
}

/** One purpose mana can be spent on. An activation purpose without a source
 * covers every activated ability (CR 602.1). */
export type ManaSpendingUse =
  | { use: "cast"; filter: ObjectFilter; from?: ZoneRef }
  | { use: "cast-commander"; whose: PlayerRef }
  | { use: "activate"; source?: ObjectFilter };

/** CR 106.6: besides restrictions, produced mana can carry an additional
 * effect on the spell or ability it is spent on, or a delayed triggered
 * ability that triggers when it is spent (603.7a). Each unit of the produced
 * mana carries its own copy (106.6a); "any-of-that" is the printed wording
 * for several units. In `effects`, "it" / "that spell" / "that creature"
 * denote the spell or ability the mana was spent on and, for a permanent
 * spell, the permanent it becomes; they stay symbolic. */
export type ManaSpendingRider =
  | { rider: "effect"; mana: "that" | "any-of-that"; spentOn: ManaSpendingUse[]; effects: Sentence[] }
  | { rider: "delayed-trigger"; spentOn: ManaSpendingUse[]; effects: Sentence[] };

export interface CounterSpec {
  /** "+1/+1", "-1/-1", or a named counter: "charge", "loyalty", "stun" … */
  type: string;
}

// ---------------------------------------------------------------------------
// Static effects
// ---------------------------------------------------------------------------

/** Exceptions to a copy effect are copiable values, not later layer 4–7
 * changes (CR707.9a–d). Setting a characteristic suppresses corresponding
 * model CDAs (and a color indicator for color); additive type exceptions
 * retain those CDAs. An ability defined here may itself qualify as a CDA. */
export type CopyCharacteristicChange =
  | { change: "types"; types: TypeChange }
  | { change: "colors"; operation: "set" | "add"; colors: Color[] }
  | { change: "power-toughness"; power: Amount; toughness: Amount }
  | { change: "abilities"; abilities: GainedAbility[] };

/** CR614.12,707.2b–c,707.5: one replacement per entering object, with
 * copiable values fixed when the copy first applies, before actual entry.
 * Model choices are not targets. References in model/filter are evaluated
 * in the source's scope; changes and entry state describe each entrant.
 * Declining the optional copy also declines its entry modifications.
 * Copied entry replacements still apply and entry triggers see the copy. */
export interface EntryCopy {
  what: ObjectRef;
  model: { model: "choose"; who: PlayerRef; filter: ObjectFilter; count: 1; when: "before-entry" }
    | { model: "reference"; what: ObjectRef };
  optional?: { who: PlayerRef };
  changes?: CopyCharacteristicChange[];
  /** Noncopiable entry state, printed before "as a copy" (Vesuva). */
  entry?: { tapped: true };
  /** Additional-effect exceptions are noncopiable and do not happen if a
   * further copy effect applies afterward (CR707.9e, Altered Ego). */
  entryExceptions?: { exception: "counters"; counter: CounterSpec; count: Amount; additional?: true }[];
  duration?: Duration;
}

export type StaticEffect =
  | { static: "group"; effects: StaticEffect[] }
  | ({ static: "entry-copy" } & EntryCopy)
  /** Recurring replacement while this static ability functions. No pending
   * one-shot skip is created on resolution (CR611.3,614.10). */
  | { static: "skip-time"; who: PlayerRef; what: SkippableTime }
  /** CR 711.2/4-6: one printed level symbol is a static ability. While the
   * source is on the battlefield and has the indicated inclusive number of
   * level counters, it has this base P/T (if printed) and these abilities.
   * The symbol and its complete contents remain copiable printed abilities
   * even when inactive; counters and the evaluated characteristics do not.
   * Earlier abilities and the uppermost P/T in source metadata stay intact.
   * Noncreature levelers can omit the P/T box (official MB2 release notes). */
  | { static: "level-band"; range: { min: number; max?: number }; basePT?: { power: number; toughness: number }; abilities: Ability[] }
  | { static: "replacement"; replacement: ReplacementInstruction }
  | ({ static: "cost-modification"; scope: CostModificationScope } & CostModification)
  | { static: "as-enters"; what: ObjectRef; effects: Sentence[] }
  /** CR 708.11: an "As [this permanent] is turned face up" ability is applied
   * while it is being turned face up, not afterward and not as a trigger.
   * `alsoEnters` is "As this creature enters or is turned face up". */
  | { static: "as-turned-face-up"; what: ObjectRef; effects: Sentence[]; alsoEnters?: true }
  /** Optional payment/reveal before entry, with a declined-payment entry
   * modification. This is one replacement choice, not a tap instruction.
   * CR 614.1c, 614.12a–b. */
  | { static: "entry-choice"; what: ObjectRef; who: PlayerRef; costs: Cost[]; declined: { tapped: true } }
  | { static: "prevent-damage"; prevention: DamagePrevention }
  | { static: "prohibit"; prohibition: GameProhibition }
  | { static: "cast-limit"; limit: CastLimit }
  /** CR 603.2d: whenever a matching ability would trigger, it triggers
   * `additional` more times. Only abilities the matching sources themselves
   * have are affected, never delayed or reflexive triggers they create, and
   * the effect neither reapplies to its own additional triggers nor combines
   * multiplicatively with another such effect. `cause` restricts the event
   * that made the ability trigger (absent: any event). `condition` is the
   * printed "while" test, evaluated as the ability triggers. */
  | { static: "additional-triggers"; abilities: ObjectFilter; cause?: Trigger; additional: number; condition?: Condition }
  /** CR 603.2/603.6a: an event matching `cause` doesn't make the matching
   * abilities trigger at all. The same abilities still trigger on other
   * events, and no ability is removed or countered. */
  | { static: "trigger-suppression"; abilities: ObjectFilter; cause: Trigger }
  | { static: "play-permission"; permission: CardPlayInstruction }
  | { static: "look-permission"; permission: LookPermission }
  /** CR 113.6e, 601.3: restricts when this particular object can begin to be
   * cast. It functions in every zone the card could be cast from and on the
   * stack; it is never an activation restriction or a condition checked on
   * resolution. `windows` are alternatives (any one suffices); `condition`
   * must also be true at that moment. */
  | { static: "cast-restriction"; what: Extract<ObjectRef, { ref: "self" | "this" }>; windows?: CastWindow[]; condition?: Condition }
  | { static: "additional-land-plays"; allowance: LandPlayAllowance }
  /** CR 401.5–6, 701.20a: cards in this hidden zone position stay revealed
   * to all players while the ability functions; it is not a one-shot reveal
   * instruction. A newly exposed library top is revealed only after the
   * current casting, activation, or special action has finished (401.5).
   * Each affected player's own zone is revealed. */
  | { static: "play-revealed"; who: PlayerRef; what: "library-top" | "hand" }
  /** CR 903.3a / 113.6n: eligibility during deck construction, not an
   * in-game designation, characteristic, or controller-based permission. */
  | { static: "commander-eligibility"; what: Extract<ObjectRef, { ref: "self" | "this" }>; during: "deck-construction" }
  /** CR 103.6a: a pregame action with this card from its owner's opening
   * hand, taken after mulligans in turn order. `who` may put the card onto
   * the battlefield so the game begins with it there; this is a permission,
   * never a resolving instruction or a replacement of a later entry. */
  | { static: "opening-hand-action"; what: Extract<ObjectRef, { ref: "self" | "this" }>; who: PlayerRef; action: "begin-on-battlefield" }
  /** CR 100.2a / 113.6n: replaces the four-copy limit for cards with this
   * English name during deck construction (deck and sideboard combined,
   * 100.4a). Absent maximum means any number. The name is the printed
   * literal, not a reference to whatever this object is currently named. */
  | { static: "deck-copy-limit"; name: { name: "literal"; value: string }; maximum?: number; during: "deck-construction" }
  /** CR 502.3: the chooser may omit this object from the simultaneous
   * turn-based untap action; this never prohibits untapping by an effect. */
  | { static: "optional-untap"; who: PlayerRef; what: ObjectRef; step: { occurrence: "each"; owner: PlayerRef } }
  /** CR 118.9: an optional casting payment, never a resolution instruction. */
  | { static: "alternative-cost"; who: PlayerRef; spells: ObjectRef; costs: Cost[]; condition?: Condition }
  | { static: "player-abilities"; who: PlayerRef; abilities: GainedAbility[] }
  /** CR 613.1b (layer 2): a continuous control-changing effect that lasts
   * while this static ability functions. It is not a one-shot gain-control
   * instruction and creates no duration of its own (CR 611.3a). */
  | { static: "control"; who: PlayerRef; what: ObjectRef }
  | {
      static: "modify";
      what: ObjectRef;
      modifiers: StaticModifier[];
      condition?: Condition;
      /** A trailing condition is evaluated separately for each matched
       * object; an unqualified "it" refers to that object (CR 611.3). */
      conditionScope?: "each-affected-object";
      duration?: Duration;
      bindings?: { variable: "x"; value: Amount }[];
    }
  | { static: "cda-pt"; what: ObjectRef; stat: "power" | "toughness" | "both"; value: Amount }
  | { static: "conditional"; condition: Condition; effect: StaticEffect; otherwise?: StaticEffect }
  | { static: "cda-colors"; what: ObjectRef; operation: "set" | "add"; colors: Color[] | ChosenValueRef<"color"> }
  | { static: "cda-types"; what: ObjectRef; change: TypeChange }
  | { static: "enters-tapped"; what: ObjectRef; condition?: Condition; conditionKind?: "if" | "unless" }
  /** CR 614.1d: the matching permanents enter untapped. This contends with
   * enters-tapped replacement effects (CR 616.1); it never untaps a
   * permanent that is already on the battlefield. */
  | { static: "enters-untapped"; what: ObjectRef }
  /** CR 110.2, 614.1d, 614.12a: the permanent enters under the chosen
   * opponent's control. `who` chooses before it enters; this is neither a
   * target nor a later control-changing effect. */
  | { static: "enters-under-control"; what: ObjectRef; controller: { choice: "opponent"; who: PlayerRef } }
  /** CR 614.1c: the permanent has these abilities from the moment it enters,
   * for as long as it remains on the battlefield. They are granted by the
   * entry replacement effect and are not part of its copiable values. */
  | { static: "enters-with-abilities"; what: ObjectRef; abilities: GainedAbility[] }
  | { static: "untap-step-suppression"; what: ObjectRef; step: UntapStepWindow }
  | { static: "enters-with-counters"; what: ObjectRef; counter: CounterSpec; count: Amount; additional?: true; multiplier?: Amount; condition?: Condition; conditionKind?: "if" | "unless";
      /** A printed "where X is" definition, evaluated as the permanent would
       * enter (CR 614.12); it is not the X of the permanent's mana cost. */
      bindings?: { variable: "x"; value: Amount }[] }
  | { static: "cant"; what: ObjectRef; action: CantAction; condition?: Condition; conditionKind?: "if" | "unless" }
  | { static: "combat-rule"; what: ObjectRef; rule: CombatRule }
  | { static: "must-attack"; what: ObjectRef };

/** Modifiers in one static.modify are parts of one continuous effect. Once
 * its affected set is established in an earlier layer, later-layer parts
 * retain that set (CR 613.6); they are not independently reselected. */
export type StaticModifier = { forEach?: ObjectFilter; multiplier?: Amount;
  /** Dynamic quantities are evaluated for each affected object. "It" and
   * "them" inside those quantities denote that object, not the source or
   * the entire affected collection. */
  valueScope?: "each-affected-object"; condition?: Condition; conditionScope?: "each-affected-object" } & (
  | { modifier: "combat-rule"; rule: CombatRule }
  | { modifier: "cant"; action: CantAction }
  /** CR 701.15a-b: the affected permanents are goaded for as long as this
   * static ability applies; the goading player is its controller. */
  | { modifier: "goaded" }
  | { modifier: "pt"; power: SignedAmount; toughness: SignedAmount }
  | ({ modifier: "base-pt" } & (
      | { power: Amount; toughness?: Amount }
      | { power?: Amount; toughness: Amount }))
  | { modifier: "colors"; operation: "set" | "add"; colors: Color[] | ChosenValueRef<"color"> }
  | { modifier: "types"; change: TypeChange }
  | { modifier: "lose-abilities"; removal: AbilityRemoval }
  /** CR 113.11 "can't have": see the cant-have-abilities effect. */
  | { modifier: "cant-have-abilities"; abilities: KeywordInstance[] }
  | { modifier: "abilities"; abilities: GainedAbility[] });

/** CR 608.2d, 611.2c/d: one value announced by `who` as the resolving effect
 * is created, shared by every affected object and fixed for its duration.
 * Neither a target nor a linked earlier choice (607.2d). A color choice of
 * one-or-more selects a nonempty set of colors; colorless is not a color. */
export type FreshCharacteristicChoice<Category extends "color" | "creature-type" | "basic-land-type"> = {
  choice: "fresh-choice"; category: Category; who: PlayerRef; when: "effect-created";
} & (Category extends "color" ? { quantity: "one" | "one-or-more" } : unknown);

export type SubtypeFamily = "artifact" | "enchantment" | "land" | "planeswalker" | "spell" | "creature" | "battle" | "dungeon";
/** CR 205.1a-b / 305.7: type operations preserve the relevant family. A set
 * of basic land types invokes the rule's intrinsic mana/ability changes;
 * adding those types retains existing rules text. Setting card types still
 * retains instant/sorcery types as required by CR 205.1a. */
export interface TypeChange {
  cardTypes?: { operation: "set" | "add" | "remove"; types: string[] };
  subtypes?: { family: SubtypeFamily; operation: "set" | "add" | "remove";
    types: string[] | { all: true; basic?: boolean } | FreshCharacteristicChoice<"creature-type" | "basic-land-type"> }[];
  /** A symbolic earlier subtype choice (CR 205.3e, 607.2d). Its subtype
   * family is that of the linked choice; setting replaces only that family
   * and a chosen basic land type still invokes CR 305.7. */
  chosenSubtype?: { operation: "set" | "add"; choice: NonNullable<ObjectFilter["chosenType"]> };
  supertypes?: { operation: "add" | "remove"; types: string[] };
}

/** CR 118.7 and 601.2f: a payment adjustment does not change mana cost or
 * mana value. A caster or activation source is not an object controller. */
export type CostModificationScope =
  | { action: "cast"; spells: ObjectRef; caster?: PlayerRef; from?: ZoneRef;
      /** Cast from any zone except this one ("from anywhere other than your hand"). */
      notFrom?: ZoneRef; occurrence?: CostModificationOccurrence }
  | { action: "activate"; source?: ObjectRef; activator?: PlayerRef;
      /** An ability's targets are distinct from the object bearing it. */
      abilities?: ObjectRef; keyword?: "equip" | "boast" | "cycling" | "ninjutsu" | "exhaust" | "power-up";
      loyalty?: true; occurrence?: CostModificationOccurrence }
  /** Every payment of the named keyword cost, whichever kind of cost that
   * keyword defines: alternative (flashback 702.34a, dash 702.109a),
   * additional (buyback 702.27a), or special-action (morph 116.2b, unlock
   * 116.2m). The cost to cast a spell face down is not a morph cost. */
  | { action: "pay-cost"; cost: "flashback" | "buyback" | "dash" | "morph" | "unlock"; payer?: PlayerRef };

/** Which of the stated actor's matching casts or activations are adjusted.
 * An ordinal counts all of that actor's matching ones within each turn;
 * "next" is the first matching one after a resolving effect is created,
 * which uses that effect up. */
export type CostModificationOccurrence = { ordinal: number; period: "turn" } | { next: true };

/** Shared payment adjustment. On an activated or loyalty ability, this
 * applies to that ability's total activation cost, before it resolves.
 * On a static effect, the enclosing scope identifies affected payments. */
export interface CostModification {
  change: { direction: "increase" | "reduce"; symbols: string[] };
  multiplier?: Amount;
  bindings?: { variable: "x"; value: Amount }[];
  minimumTotalMana?: Amount;
  coloredOnly?: true;
  condition?: Condition;
}

/** Casting (CR 601) and playing (CR 701.18) remain distinct. Playing a card
 * includes playing a land or casting a spell, subject to the usual rules.
 * An alternative cost replaces only the mana cost: additional costs,
 * increases, and reductions still apply (CR 118.9d, 601.2f).
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export interface CardPlayInstruction {
  who: PlayerRef;
  action: "cast" | "play";
  what: ObjectRef;
  from?: ZoneRef | { anyOf: ZoneRef[] };
  /** Candidate cards drawn from an existing or related set, not a zone. */
  fromAmong?: ObjectRef;
  alternativeCost?: { cost: "without-mana-cost" } | { cost: "pay"; costs: Cost[] };
  additionalCosts?: Cost[];
  /** Granted to the spell as it is cast using this permission, including
   * during cost determination (CR 601.2f, 702.41). This is not an instruction
   * performed when the permission is created. No duration is invented;
   * CR 400.7a governs continuation onto a resulting permanent. */
  spellAbilities?: KeywordInstance[];
  timing?: "as-though-flash";
  condition?: Condition;
  /** Permission-specific limit, independent of general land-play limits.
   * A generic counted filter in a lasting permission is selected when the
   * permission is used, not when created; the cap covers the entire grant. */
  useLimit?: { maximum: number; period: "turn"; during: "your" | "any" }
    | { maximum: Amount; period: "permission" };
  /** Alters acceptable payment only; does not change mana actually spent. */
  manaPayment?: "any-color" | "any-type";
  riders?: PermissionRider[];
}

/** A consequence tied to using a play permission. Its sentence forms one
 * instruction with the permission (CR 608.2c), so it is never a test made
 * while the granting ability resolves. Inside a rider, `it` and "that
 * creature" denote the spell cast (or land played) this way and then the
 * permanent it becomes. */
export type PermissionRider =
  /** CR 614.1c, 614.12: modifies how that permanent enters the battlefield.
   * `appliesTo` restricts the rider to matching spells cast this way. */
  | { rider: "entry"; appliesTo?: ObjectFilter; tapped?: true; counters?: NonNullable<BattlefieldEntryModifiers["counters"]> }
  /** CR 307.5a: applies if the spell was cast this way without priority,
   * outside its controller's main phase, or while another object was on the
   * stack. The instructions are carried out as it is cast. */
  | { rider: "cast-off-sorcery-timing"; effects: Sentence[] };

/** CR 506.8: the point of a turn that bounds a casting window. Declaration
 * points mean the beginning of that step, whether or not anything is
 * declared (506.8a–b). Without a during-combat window in the same CastWindow
 * a combat point refers to the first combat phase of the turn (506.8d–e);
 * with one it applies within each combat phase (506.8c, 506.8f). */
export type CastTimingPoint = "combat" | "attackers-declared" | "blockers-declared" | "combat-damage-step" | "upkeep-step";

/** All stated parts must hold together as the spell would begin to be cast. */
export interface CastWindow {
  /** Whose turn it must be, relative to the would-be caster. Absent: any. */
  turn?: "your" | "opponent";
  /** The phase or step that must be the current one. */
  during?: "combat" | "declare-attackers" | "declare-blockers" | "upkeep" | "end-step";
  before?: CastTimingPoint;
  after?: CastTimingPoint;
}

/** CR 305.2: the number of lands a player can play during each of their own
 * turns is one plus every applicable allowance; 305.2a compares that number
 * with the lands already played. This neither plays a land nor permits
 * playing one outside the player's own main phase (305.3, 505.6b). A
 * quantified player gets the allowance on each of their own turns. */
export interface LandPlayAllowance {
  who: PlayerRef;
  additional: Amount | "unlimited";
}

/** CR 614.1, 614.5–6, 614.11, 614.16 and 616.1: these are prospective
 * events, not triggered events or ordinary resolution-time conditions.
 * A singular object selector matches each applicable object; its grammatical
 * count is not a restriction on the size of a simultaneous zone-change batch.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export type ReplacementEvent =
  | { event: "move-zone"; what: ObjectRef; from?: ZoneRef | "anywhere"; to?: ZoneRef }
  | { event: "damage"; source?: ObjectRef; to?: (ObjectRef | PlayerRef)[];
      kind: "all" | "combat" | "noncombat"; quantity?: Comparison }
  | { event: "gain-life" | "lose-life"; who: PlayerRef }
  | { event: "draw"; who: PlayerRef; quantity?: Comparison; except?: "first-in-each-draw-step" }
  | { event: "create-token"; under?: PlayerRef; tokens?: ObjectFilter;
      cause: "any" | "effect"; quantity: Comparison; match: "each-token" | "creation-event" }
  /** `on` lists alternatives for eligible recipients, not simultaneous
   * instructions to place counters on every matching recipient. */
  | { event: "put-counters"; on: (ObjectRef | PlayerRef)[]; counter: CounterSpec | "any";
      by?: PlayerRef; cause: "any" | "effect"; quantity: Comparison };

export interface ReplacementInstruction {
  event: ReplacementEvent;
  occurrence: "each" | "next";
  condition?: Condition;
  /** The choice occurs when the event would happen, not when a resolving
   * spell or ability creates the replacement effect. */
  optional?: { who: PlayerRef };
  replace:
    /** Preserve the event's source, recipients and token/counter kinds.
     * Counter changes apply separately per counter kind and recipient;
     * token multiplication preserves each kind of token in the event. */
    | { replacement: "modify-quantity"; change:
        | { operation: "multiply"; factor: number }
        | { operation: "half"; round: "up" | "down" }
        | { operation: "add" | "subtract" | "set"; amount: Amount } }
    /** Keep every original token specification; create these additional
     * tokens. An event-quantity count means TOTAL original tokens, unlike
     * multiplying each original kind's quantity in modify-quantity. */
    | { replacement: "augment-tokens"; count: Amount; token: TokenSpec }
    /** Replace characteristics, preserving original creation modifiers such
     * as tapped/attacking and the originating instruction's haste or delayed
     * exile. With an each-token event, count applies to EACH matching token.
     * Fisher's Talent: https://magic.wizards.com/en/news/feature/bloomburrow-release-notes */
    | { replacement: "replace-token-spec"; count: Amount; token: TokenSpec }
    | { replacement: "effects"; effects: Sentence[] };
}

// ---------------------------------------------------------------------------
// Conditions & durations
// ---------------------------------------------------------------------------

/** A completed movement by the stated actor, within the referenced resolving
 * instruction sequence. This is not a current-zone test, a new choice, or
 * the choice to pay an optional resolution cost (CR118.12, 608.2c/i, 609.3).
 * Definite descriptions need an antecedent; filters query historical moved
 * objects, with their characteristics at the event. */
export type PutActionSubject =
  | { match: "object"; what: ObjectRef }
  | { match: "definite-description"; filter: ObjectFilter }
  | { match: "filter"; filter: ObjectFilter; comparison: Comparison };

export type Condition =
  | { condition: "put-action-history"; who: PlayerRef; subject: PutActionSubject; to: ZoneRef; scope: "contextual" | "this-way" }
  /** The currently occurring phase, not the phase in which a spell was
   * cast. Ordinals count only matching phases of this turn (CR505.1b). */
  | { condition: "current-phase"; phase: "main" | "combat"; whose?: PlayerRef; ordinal?: number }
  | { condition: "damage-prevented"; source?: ObjectFilter }
  | { condition: "dice-result"; roll: { roll: "contextual" }; who?: PlayerRef; test: DiceResultTest; quantifier: "single" | "any" }
  /** The referenced search happened in this resolving instruction sequence;
   * whether it found a card is independent (CR 701.23b-d). */
  | { condition: "searched-zone"; who: PlayerRef; zone: ZoneRef; scope: "this-way" }
  /** CR 508.4: having attacked requires declaring an attacker; merely
   * controlling a creature put onto the battlefield attacking is insufficient. */
  | { condition: "attacked"; who: PlayerRef; during: "this-turn" | "this-combat"; with?: ObjectFilter }
  /** Passive counterpart: a creature was declared as an attacker against
   * this player during the current step (CR 508.1b). The printed text does
   * not say whether an attack on the player's planeswalker or battle
   * qualifies; this node asserts only the printed predicate. */
  | { condition: "was-attacked"; who: PlayerRef; during: "this-step" }
  /** Historical declared combat actions, distinct from current attacking or
   * blocking status. Shared-subject alternatives use explicit any-of nodes. */
  | { condition: "combat-action-history"; what: ObjectRef; action: "attacked" | "blocked"; during: "this-turn" | "this-combat" }
  /** CR 400.7d: an entering permanent can ask about its originating spell.
   * when tests the casting event's phase, never the current resolving phase. */
  | { condition: "was-cast"; what: ObjectRef; by?: PlayerRef; from?: ZoneRef; when?: { phase: PhaseName; whose: PlayerRef } }
  /** Actual damage events, not life lost or damage still marked. Singular
   * source/recipient descriptions instantiate one matching object/player;
   * total sums matching damage over the period for those same referents. */
  | { condition: "damage-history"; source?: ObjectRef; to?: ObjectRef | PlayerRef;
      damage?: "combat" | "noncombat"; quantity: "positive-event" | "total";
      comparison: Comparison; during: "this-turn" | "this-combat" }
  /** Mana actually spent, never the spell's cost or mana value (CR 609.4b).
   * Snow is a source quality rather than a mana color/type (CR 107.4h).
   * source is evaluated when that mana was produced (CR 106.3). */
  | { condition: "mana-spent-to-cast"; what: ObjectRef;
      mana: "any" | "colored" | Color | "colorless" | "snow";
      comparison: Comparison; source?: ObjectFilter }
  | { condition: "has-keyword"; what: ObjectRef; keyword: string }
  /** Each qualifying object movement counts separately. Filters and control
   * are evaluated at that movement, not using the object's current state.
   * A dies history is battlefield -> graveyard (CR 700.4). */
  | { condition: "zone-change-history";
      event: "dies" | "enters" | "leaves-battlefield" | "leaves-zone" | "put-into-zone";
      subject: { match: "filter"; filter: ObjectFilter; comparison: Comparison } | { match: "object"; what: ObjectRef };
      from?: ZoneRef | "anywhere"; to?: ZoneRef; controllerAtMove?: PlayerRef; during: "this-turn" | "this-combat" }
  | { condition: "targets"; what: ObjectRef; target: ObjectRef | PlayerRef }
  | { condition: "characteristic-relation"; what: ObjectRef; test: CharacteristicRelation }
  | { condition: "compare"; value: Amount; comparison: Comparison }
  /** Quantified players retain their quantifier: an opponent is existential,
   * while each player is tested independently, never a combined counter sum. */
  | { condition: "player-counters"; who: PlayerRef; counter: CounterSpec | "any"; comparison: Comparison }
  | { condition: "all-of" | "any-of"; conditions: Condition[] }
  | { condition: "not"; of: Condition }
  /** CR 702.33d–f: records how many kicks and the linked kicker cost, if specified. */
  | { condition: "kicked"; what: ObjectRef; comparison: Comparison; kickerCost?: Cost[] }
  /** A keyword's own linked casting/entry decision, as its rule defines it:
   * bargained (CR 702.166b), gift promised (702.174k), evidence collected
   * (701.59c), tribute paid (702.104b), escaped (702.138b), or cast for the
   * keyword's alternative cost (prowl 702.76a, surge 702.117a, madness
   * 702.35b, spectacle 702.137a). `what` is the printed spell or the
   * permanent it became (400.7d); when the text names no object, the query
   * belongs to the object bearing this linked ability (607.2). */
  | { condition: "keyword-choice"; keyword: LinkedChoiceKeyword; what?: ObjectRef }
  /** This spell was cast by paying the alternative cost its own text offers
   * with exactly these components (CR 118.9, 607.2). */
  | { condition: "alternative-cost-paid"; cost: Cost[] }
  /** Resolutions of this same ability of this source during the period,
   * counting the current one; a copy counts as the same ability (CR
   * 707.10b). True when that count is any listed ordinal. */
  | { condition: "ability-resolution-ordinal"; ordinals: number[]; during: "this-turn" }
  /** Activations of this same ability of this source during the period,
   * including the one resolving (CR 602.2). */
  | { condition: "ability-activation-count"; comparison: Comparison; during: "this-turn" }
  /** Gross life gained/lost over the period, not a net life-total change. */
  | { condition: "life-change"; who: PlayerRef; change: "gained" | "lost"; comparison: Comparison; during: "this-turn" }
  /** Completed actions by one matching player during the period. The
   * comparison counts objects (sacrifice, discard, draw, create-token,
   * cycle) or occurrences (the rest). Filters describe each object as the
   * action happened, not its current state (CR 608.2i). */
  | { condition: "player-action-history"; who: PlayerRef; action: PlayerHistoryAction; comparison: Comparison;
      during: "this-turn" | "last-turn" }
  | { condition: "cast-count"; who: PlayerRef | "all-players-combined"; what: ObjectFilter; comparison: Comparison; during: "this-turn" | "last-turn"; from?: ZoneRef }
  /** time "previous" is the referenced object's last existence before the
   * instruction just applied to it moved or destroyed it (CR 608.2h). */
  | { condition: "control"; who: PlayerRef; what: ObjectRef; time?: "previous" }
  | { condition: "control-count"; who: PlayerRef; of: ObjectFilter; comparison: Comparison }
  /** The same quantity measured for two player operands. Each operand keeps
   * its printed quantifier: an opponent / a player is existential, each
   * opponent / each other player universal. Every pair is compared
   * separately; quantities of several players are never summed. */
  | { condition: "player-comparison"; who: PlayerRef; quantity: PlayerQuantity; op: "gt" | "lt";
      than: PlayerRef | { player: "each-other-player" } }
  /** There exists a subset of the controlled eligible objects of at least
   * minimum size satisfying all group constraints. This makes no choice and
   * need not include every controlled object. Names obey CR201.2b, including
   * objects with multiple names and nameless objects. */
  | { condition: "controls-subset"; who: PlayerRef; of: ObjectFilter; minimum: Amount; groupConstraints: GroupConstraint[] }
  /** CR 701.38: the result of the preceding vote. "more" is strictly more
   * votes than the other choice; "most" is strictly the most votes;
   * "-or-tied" also holds when the vote is tied (for "more": "or the vote is
   * tied"; for "most": "or is tied for the most votes"). */
  | { condition: "vote-result"; choice: string; result: "more" | "more-or-tied" | "most" | "most-or-tied" }
  /** CR 701.30d: the contextual preceding clash was won by `who`. */
  | { condition: "clash-result"; who: PlayerRef; result: "won" }
  | { condition: "you-do" }
  | { condition: "you-dont" }
  /** CR 118.12: the player could not begin the mandatory action of the
   * immediately preceding instruction. Not a declined optional action. */
  | { condition: "preceding-action"; who: PlayerRef; outcome: "unable" }
  | { condition: "pays"; who: PlayerRef; cost: Cost[] }
  | { condition: "status"; what: ObjectRef; status: string; time?: "previous" }
  | { condition: "is-filter"; what: ObjectRef; filter: ObjectFilter; time?: "previous" }
  | { condition: "count"; of: ObjectFilter; comparison: Comparison }
  | { condition: "life-total"; who: PlayerRef; comparison: Comparison }
  | { condition: "your-turn" }
  | { condition: "not-your-turn" }
  /** The active player is one of the stated players (CR 102.1). */
  | { condition: "turn-of"; who: PlayerRef }
  /** Monarch (CR 725) and the initiative (726) have at most one holder; any
   * number of players can have the city's blessing (702.131c). `a-player`
   * asks whether any player currently holds the designation. */
  | { condition: "designation"; who: PlayerRef; designation: PlayerDesignation }
  /** CR 731.1: a designation of the game itself; "neither" holds until it
   * first becomes day or night. */
  | { condition: "day-night"; state: "day" | "night" | "neither" }
  /** CR 309.7: this player has completed at least one dungeon this game. */
  | { condition: "completed-dungeon"; who: PlayerRef }
  /** CR 700.8c: four creatures in that player's party. */
  | { condition: "full-party"; who: PlayerRef }
  | { condition: "remains"; what: ObjectRef; zone: ZoneRef }
  | { condition: "cards-in-hand"; who: PlayerRef; comparison: Comparison };

export type PlayerDesignation = "monarch" | "initiative" | "city's-blessing";

export type LinkedChoiceKeyword = "bargain" | "gift" | "collect-evidence" | "tribute" | "escape"
  | "prowl" | "surge" | "madness" | "spectacle";

/** A per-player quantity for player-comparison. `of` has no controller of
 * its own; each compared player supplies it. */
export type PlayerQuantity =
  | { quantity: "controlled-count"; of: ObjectFilter }
  | { quantity: "cards-in-hand" }
  | { quantity: "life-total" };

/** CR 701.21 (sacrifice), 701.9 (discard), 121 (draw), 111 (tokens),
 * 702.29 (cycle), 305 (land plays), 700.13 (crimes), 701.22/701.25. */
export type PlayerHistoryAction =
  | { action: "sacrifice" | "discard" | "create-token" | "cycle"; what: ObjectFilter }
  | { action: "draw" | "play-land" | "commit-crime" | "scry" | "surveil" };

export type Duration =
  | { duration: "end-of-turn" }
  | { duration: "end-of-combat" }
  | { duration: "end-of-your-next-turn" }
  /** Expires as the next end step of your turn begins, not in its cleanup. */
  | { duration: "your-next-end-step" }
  /** Expires as the beginning of your next upkeep begins (CR 502.1, 611.2). */
  | { duration: "your-next-upkeep" }
  | { duration: "this-turn" }
  | { duration: "this-combat" }
  | { duration: "as-long-as"; condition: Condition }
  | { duration: "your-next-turn" };

/** CR 502.3 and 611.2: an untap-step restriction is not a prohibition on
 * untapping at other times. Controller ownership is evaluated separately
 * for each affected permanent, rather than picking one shared controller.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export interface UntapStepWindow {
  occurrence: "each" | "next";
  owner: PlayerRef | "each-affected-objects-controller";
}

/** CR 401.2/5, 701.20e, 708.5: a private information permission, not a
 * reveal or a one-shot look instruction. Queries are evaluated whenever
 * used, so a top card or an eligible face-down permanent may change.
 * `topLibraryChanges` applies ONLY to top-of-library queries in `what`:
 * a newly exposed top card cannot be looked at until the current casting,
 * activation, or special-action procedure has finished. It imposes no such
 * restriction on a separate face-down-object query in the same permission.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export interface LookPermission {
  who: PlayerRef;
  what: ObjectRef[];
  timing: "any-time";
  topLibraryChanges?: "after-casting-activation-or-special-action";
}

// ---------------------------------------------------------------------------
// References: players, objects, filters
// ---------------------------------------------------------------------------

/** Identity of one printed target selector (CR 601.2c), not a selected game
 * object. Reusing a parsed subject preserves this identity; a repeated target
 * phrase has a different identity even when the filters are identical.
 * IDs are local to one parsed face and emitted only with source provenance. */
export interface TargetSelection { selectionId?: string }

/** A game value is chosen, not an object bearing that value. Exact option
 * lists retain their namespace; a chosen subtype need not be a creature type. */
export type ChosenValueCategory = "color" | "card-type" | "card-name" | "player";
/** CR 607.2d: refers to the linked choice, not a fresh selection or a literal
 * value. Binding must identify the originating ability/choice instance. */
export interface ChosenValueRef<Category extends ChosenValueCategory = ChosenValueCategory> {
  choice: "chosen-value";
  category: Category;
  which: "chosen" | "last-chosen";
}

export type ValueChoice =
  | { category: "color"; count?: number; excluding?: Color[]; options?: Color[] }
  | { category: "card-type"; excluding?: string[] }
  | { category: "creature-type" | "basic-land-type" | "land-type" | "card-name" | "number" }
  | { category: "type"; options: { type: "card-type" | "subtype"; value: string }[] };

export type PlayerRef =
  | { player: "prevention-event-player"; role: "recipient" }
  | { player: "you" }
  | { player: "a-player" }
  /** The same selection/quantifier with additional eligibility. A target in
   * base remains a target; this wrapper does not create a resolution choice. */
  | { player: "filtered"; base: PlayerRef; filter: PlayerFilter }
  | { player: "replacement-event-player"; role: "actor" | "recipient" }
  | { player: "each-player" }
  | { player: "each-opponent" }
  | ({ player: "target-player" } & TargetSelection)
  | ({ player: "target-opponent" } & TargetSelection)
  | { player: "an-opponent" }
  | { player: "that-player" }
  /** "That player or that planeswalker's controller": one earlier
   * object-or-player antecedent. This is that antecedent if it is a player,
   * otherwise the controller of that object; exactly one case applies. It is
   * neither a choice nor a pair of players (CR 608.2c). */
  | { player: "that-player-or-controller"; of: ObjectRef }
  /** Definite contextual reference, not a new player choice. Its original
   * event/choice role must be established before execution. */
  | { player: "the-player" }
  /** The printed chosen/last-chosen participant. May refer to an announced
   * target, a resolution choice, or a linked pre-entry choice; this node
   * never chooses a new player or guesses the originating ability. */
  | { player: "chosen-player"; choice: ChosenValueRef<"player"> }
  | { player: "they" }
  /** One action applies once per printed participant occurrence. Coincident
   * players are NOT deduplicated: Angel of Destiny doubles the life gain as
   * one event. Draw order remains CR121.2c/d, other actions CR608.2e/f. */
  | { player: "group"; members: PlayerRef[]; multiplicity: "per-member-occurrence" }
  /** The player this Aura enchants, independent of the Aura's controller. */
  | { player: "enchanted-player" }
  | { player: "controller"; of: ObjectRef }
  | { player: "owner"; of: ObjectRef }
  | { player: "defending-player" };

/** Existence of a positive actual damage event involving the candidate,
 * never damage currently marked or life lost (CR 120.3/7). Query operands
 * constrain the historical counterpart; they do not choose new participants.
 * Historical source incarnation/controller binding is a separate obligation. */
export type DamageHistoryPredicate = {
  damage?: "combat" | "noncombat";
  window: { during: "this-turn" | "this-combat" } | { event: "this-way" };
} & (
  | { role: "recipient"; source?: ObjectRef }
  | { role: "source"; to?: ObjectRef | PlayerRef }
);

/** Restrictions are conjunctive. Each candidate player supplies the subject
 * of hand/life/control predicates; no shared player is picked for a set. */
export interface PlayerFilter {
  allOf?: PlayerFilter[];
  damageHistory?: Extract<DamageHistoryPredicate, { role: "recipient" }>;
  handSize?: Comparison;
  lifeTotal?: Comparison;
  /** Each candidate player's counters are counted independently (CR 122.1). */
  counterConstraints?: { counter: CounterSpec | "any"; comparison: Comparison }[];
  controls?: { filter: ObjectFilter; comparison: Comparison };
  otherThan?: PlayerRef;
}

/** Conjunctive restrictions on the complete selected group. These never
 * restrict a single candidate independently. Different names uses CR201.2b:
 * every object has at least one name and no pair shares a name. Sums include
 * signed values (CR107.1b); zero selected objects have an empty sum of zero. */
export type GroupConstraint =
  | { constraint: "shared-value"; characteristic: SharedCharacteristic }
  | { constraint: "attribute-sum"; attribute: "power" | "toughness" | "mana-value"; comparison: Comparison }
  | { constraint: "pairwise-different"; property: "name" | "power" | "mana-value" };

/** A separately counted, untargeted card-extraction slot. A fixed count is
 * not weakened to up-to by hidden-zone search's failure-to-find rule. */
export interface CardExtractionSlot {
  filter: ObjectFilter;
  count: Amount;
  upTo?: boolean;
  groupConstraints?: GroupConstraint[];
}

export type ObjectRef =
  | { ref: "prevention-event-object"; role: "source" | "recipient" }
  | { ref: "self" }                                          // ~
  | { ref: "replacement-event-object"; role: "affected" | "source" | "recipient" }
  | { ref: "it" }
  | { ref: "them" }
  /** Printed "the copy/copies". Requires a copy-result binding; proximity
   * alone must not substitute an arbitrary previous object or token. */
  | { ref: "copy-result"; number: "singular" | "plural" }
  /** Printed chosen-object reference, not a new selection or target. Its
   * origin may be an announcement target, resolution/search choice, or a
   * linked ability's choice. Contextual origin must be bound explicitly;
   * neither the nearest object nor a new target is inferred. `filter` retains
   * the printed descriptor for that binding, not a new eligibility test:
   * "the chosen creature" still identifies it after its types change.
   * Printed postqualifiers also require contextual interpretation; this node
   * does not implicitly reselect or recheck the chosen object's legality. */
  | { ref: "choice-result"; number: "singular" | "plural"; filter: ObjectFilter; which: "chosen" | "last-chosen"; scope: "contextual" }
  /** The unique object of this lexical trigger occurrence, proven by the
   * parser for an immediate first copy instruction; never the latest noun. */
  | { ref: "trigger-event-object"; event: "cast" | "activate" }
  | { ref: "library-cards"; position: "top" | "bottom"; count: Amount; of: ZoneRef }
  | { ref: "this"; noun: string }                            // "this creature", "this spell"
  | { ref: "that"; filter: ObjectFilter; plural?: true }      // "that card", "those creatures"
  /** Select the stated number from an established pool at the enclosing
   * instruction's choice time (payment or resolution), not a new target. */
  | { ref: "from-pool"; pool: ObjectRef; count: Amount; upTo?: true; groupConstraints?: GroupConstraint[]; random?: true }
  /** Distinct card allocations for search/reveal/move instructions, e.g.
   * "a creature card and/or an enchantment card". This is not a filter OR
   * under one count, nor reusable representative roles (Tragic Arrogance).
   * The chosen members' counts remain independent of the outer may choice.
   * A shared source applies to every member. CR608.2c/d, 701.23b; Kaalia's
   * M20 ruling assigns a multitype card to just one extraction slot. */
  | { ref: "card-extraction-group"; members: CardExtractionSlot[]; combine: "all" | "any-nonempty"; allocation: "distinct-objects";
      source?: { kind: "pool"; pool: ObjectRef } | { kind: "zone"; zone: ZoneRef } }
  | ({ ref: "target"; filter: ObjectFilter; count: Amount; upTo?: boolean; groupConstraints?: GroupConstraint[]; random?: true } & TargetSelection)
  /** CR 115.4: "any target", "another target", "two targets". `other` is the
   * unresolved lexical contrast, exactly as ObjectFilter.other. */
  | ({ ref: "any-target"; count?: Amount; upTo?: boolean; other?: true; random?: true } & TargetSelection)
  /** Symbolic event/linked referent; antecedent resolution is a later pass. */
  | { ref: "related"; filter: ObjectFilter; relation: ObjectRelation; number: "singular" | "plural" }
  /** Complement of the selected objects in the contextual or stated pool. */
  | { ref: "rest"; of?: ObjectRef }
  /** All cards in the named player's personal zone, not the zone itself. */
  | { ref: "zone-cards"; zone: "graveyard" | "hand" | "library"; owner: PlayerRef }
  /** CR 702.124e: if both commanders qualify, choice occurs as the effect applies. */
  | { ref: "commander"; owner: PlayerRef; count: Amount }
  | { ref: "each"; filter: ObjectFilter }
  /** CR 701.38: the objects that received the most votes in the preceding
   * vote, including objects tied for the most. */
  | { ref: "vote-leaders"; filter: ObjectFilter }
  | { ref: "all"; filter: ObjectFilter }
  /** `random`: the game selects the stated number at random from the matching
   * objects ("a card at random from your hand"); no player chooses. */
  | { ref: "filter"; filter: ObjectFilter; count: Amount; upTo?: boolean; groupConstraints?: GroupConstraint[]; random?: true } // "a creature", "up to seven lands"
  | { ref: "equipped" | "enchanted"; noun: string };         // "equipped creature"

/**
 * Structured card/object filter. All fields and ordinary array entries are
 * conjunctive. Explicit anyOf branches represent alternatives.
 */
export type ObjectRelation = {
  /** Event time, distinct from an enclosing effect or permission duration. */
  period?: "this-turn";
  who?: PlayerRef;
} & (
  | { relation: "event-result"; event: "exile" | "sacrifice" | "discard" | "reveal" | "destroy"; scope: "contextual" | "this-way" }
  | { relation: "linked-exile"; source: ObjectRef }
);

/** Characteristic-set intersections, never spelling comparisons or a count
 * of printed strings. Multiple names follow CR201.2a/c; colorless objects
 * share no color. Creature/land types remain distinct subtype categories. */
export type SharedCharacteristic = "name" | "color" | "card-type" | "creature-type" | "land-type";
export type CharacteristicOperand =
  | { operand: "object"; what: ObjectRef }
  | { operand: "matching-set"; filter: ObjectFilter; quantifier: "any" | "every" };
export type CharacteristicRelation =
  | { relation: "shared-value"; characteristic: SharedCharacteristic; with: CharacteristicOperand }
  /** Candidate must have a name and share none with the compared object(s).
   * This is NOT the negation of shared-name for a nameless candidate. */
  | { relation: "different-name"; from: CharacteristicOperand }
  | { relation: "not"; of: CharacteristicRelation };

export interface ObjectFilter {
  characteristicRelations?: CharacteristicRelation[];
  allOf?: ObjectFilter[];
  damageHistory?: DamageHistoryPredicate;
  anyOf?: ObjectFilter[];
  /** All listed card types: "artifact creature". */
  types?: string[];
  /** "noncreature", "nonland" … */
  nonTypes?: string[];
  /** Object class this filter selects over. */
  cls?: "permanent" | "card" | "spell" | "token" | "ability" | "source";
  /** Ability identity is separate from its source object's characteristics. */
  abilityKind?: "activated" | "triggered" | "static" | "spell";
  manaAbility?: boolean;
  loyaltyAbility?: boolean;
  /** Canonical keyword whose rules define this activated ability, e.g. an
   * exhaust ability (CR 702.177a) or a boast ability (702.142a). */
  abilityKeyword?: string;
  /** The ability's single source must match this operand. A query operand
   * defines eligible sources; it does not instruct anyone to choose a source. */
  abilitySource?: ObjectRef;
  /** Directional relations of the filtered object, not its controller.
   * A query/set operand tests membership: at least one related object matches.
   * Explicit universal or numeric relation quantifiers are unsupported. */
  attachedTo?: ObjectRef | PlayerRef;
  blocking?: ObjectRef;
  blockedBy?: ObjectRef;
  /** CR 508.1b, 506.4c: currently attacking one of these players or
   * permanents; an attacker whose planeswalker/battle left combat matches none. */
  attacking?: CombatDefender[];
  /** Permanent card/spell: has a permanent card type, independent of zone. */
  permanent?: true;
  /** CR 903.3: persistent card designation, not a card type or subtype. */
  commander?: true;
  subtypes?: string[];
  nonSubtypes?: string[];
  supertypes?: string[];
  nonSupertypes?: string[];
  colors?: Color[];
  nonColors?: Color[];
  colorless?: boolean;
  multicolored?: boolean;
  monocolored?: boolean;
  nonToken?: boolean;
  /** "attacking-alone"/"blocking-alone" are the CR 506.5 states: attacking
   * or blocking while no other creature is. */
  status?: string[];                                         // tapped, attacking …
  /** A declared attacker earlier in the explicit window; a creature put
   * onto the battlefield attacking has not attacked (CR508.4). */
  combatHistory?: { during: "this-turn" | "this-combat" } & (
    /** negated: was not declared as an attacker in the window. */
    | { action: "attacked"; negated?: true }
    /** CR 509.1g, 509.4: was declared as a blocker or made to block by an
     * effect; never by entering the battlefield blocking. attacker limits
     * what it blocked (membership for a set operand). */
    | { action: "blocked"; attacker?: ObjectRef }
    /** CR 509.1h: became a blocked creature; blocker limits by what. */
    | { action: "was-blocked"; blocker?: ObjectRef });
  /** Unresolved lexical "another"/"other". Its excluded antecedent is
   * contextual; it must never be assumed to mean the source by default. */
  other?: boolean;
  /** Explicit exclusion, bound where the local grammar establishes it.
   * affected-object is rebound for each object a modifier applies to. */
  otherThan?: ObjectRef | { ref: "affected-object" };
  /** CR 702.33d: a spell whose controller declared a kicker payment. */
  kicked?: true;
  /** Symbols printed in the object's mana cost, e.g. ["X"] (CR 107.3, 202.1). */
  manaCostContains?: string[];
  control?: "you" | "opponent" | "not-you" | "that-player";
  /** A specifically referenced controller, including a targeted player. */
  controlledBy?: PlayerRef;
  /** Historical/proposed casting action, separate from control/current zone. */
  cast?: { by: PlayerRef; from?: ZoneRef };
  /** Literal name is unchanged when the source's name changes (CR 201.5). */
  name?: { name: "literal"; value: string };
  /** Symbolic prior type choice (CR 607.2d). Bare "chosen type" does not
   * determine whether that choice was a card type or a subtype. */
  chosenType?: { choice: "chosen-type"; kind?: "card" | "creature" | "land" | "artifact" | "enchantment" | "planeswalker" };
  chosenColor?: ChosenValueRef<"color">;
  own?: "you" | "opponent" | "not-you";
  zone?: ZoneRef;
  /** CR 400.11: in none of the game's zones (a sideboard, 400.11a). Outside
   * the game is not a zone, so this is deliberately not a ZoneRef. */
  outsideGame?: true;
  /** In any zone other than this one ("that aren't on the battlefield"). */
  outsideZone?: ZoneRef;
  properties?: PropertyConstraint[];
  /** "that targets …" */
  targets?: ObjectRef | PlayerRef;
  /** "…or player": the filter unions in players ("target creature or player"). */
  orPlayer?: boolean;
  /** "…or opponent": unions in only the opponents of the spell or ability's
   * controller ("target opponent or planeswalker"). The object branch is not
   * restricted by controller (CR 102.2–3, 115.1). Never set with orPlayer. */
  orOpponent?: boolean;
  /** Validated canonical keyword names; every listed ability is required. */
  withAbility?: string[];
  /** Every listed keyword must be absent. */
  withoutAbility?: string[];
  /** Conjunctive counter-count requirements on the selected object itself. */
  counterConstraints?: { counter: CounterSpec | "any"; comparison: Comparison }[];
  relation?: ObjectRelation;
  /** Selection is limited to this previously established pool of objects. */
  fromAmong?: ObjectRef;
}

export interface PropertyConstraint {
  property: "power" | "toughness" | "mana-value";
  comparison: Comparison;
}

export interface Comparison {
  op: "eq" | "le" | "ge" | "lt" | "gt";
  value: Amount;
}

export type ZoneRef = {
  zone: Zone;
  position?: "top" | "bottom";
  /** CR 401.7: Nth card counted from `position` ("third from the top" is 3);
   * absent means that end itself. A shorter library places it on the bottom. */
  depth?: number;
  /** CR 400.1: libraries, hands and graveyards are per player, so plural zone
   * phrases quantify over owners. "each-player"/"each-opponent" distribute the
   * enclosing instruction over every such zone ("each graveyard");
   * "all-players"/"all-opponents" pool those zones' contents ("all graveyards",
   * "your opponents' graveyards"); "any" is one unspecified zone per object
   * ("a graveyard"), while "any-single" additionally requires everything the
   * enclosing selection or count covers to share that one zone ("a single
   * graveyard"). */
  owner?: "your" | "their" | "its-owner" | "each-player" | "all-players" | "any" | "any-single" | "an-opponent" | "each-opponent" | "all-opponents" | "that-player"
    | PlayerRef;
};

/** One search instruction, with a single found-card selection across the
 * chosen zones (CR 701.23). Zone selection is separate from failing to find
 * a qualifying card in a hidden zone. The stated owner applies to each zone. */
export interface SearchZones {
  selection: "all" | "one" | "one-or-more";
  of: ZoneRef[];
}

export type Zone = "battlefield" | "graveyard" | "library" | "hand" | "exile" | "stack" | "command";

export type Color = "white" | "blue" | "black" | "red" | "green";

// ---------------------------------------------------------------------------
// Amounts
// ---------------------------------------------------------------------------

export type Amount =
  /** The exact originating roll(s) and choice of retained results still need
   * binding. Numeric results exclude planar dice (706.7); no last-number
   * heuristic supplies this contextual reference. */
  | { amount: "dice-result"; roll: { roll: "contextual" }; measure: "result" | "total" }
  | { amount: "fixed"; value: number }
  /** CR 701.38: the number of votes cast for the named choice of the
   * preceding vote. */
  | { amount: "vote-count"; choice: string }
  /** CR 702.179f: a player's speed; with no speed it is 0 for this purpose. */
  | { amount: "speed"; who: PlayerRef }
  | { amount: "replacement-event-quantity" }
  | { amount: "prevention-event-quantity"; quantity: "incoming" | "prevented" }
  | ({ amount: "event-quantity" } & EventQuantity)
  | { amount: "x" }
  | { amount: "chosen"; minimum: number; maximum?: Amount }
  | { amount: "count"; of: ObjectFilter }                    // "the number of Elves you control"
  /** CR 700.8a–b: the number of creatures in that player's party, computed
   * by the game to the highest result from zero to four. */
  | { amount: "party-size"; who: PlayerRef }
  /** Distinct creature incarnations declared as attackers in this window,
   * across all players. Repeated attacks by the same incarnation count once;
   * leaving the battlefield or ceasing to be a creature does not erase the
   * recorded attack. Entering attacking is excluded (CR400.7,508.4). This is
   * a historical set cardinality, not a query of current battlefield objects. */
  | { amount: "combat-history-count"; action: "attacked"; during: "this-turn" | "this-combat"; distinct: "object-incarnations" }
  /** Cardinality of the UNION of represented values, not number of objects.
   * Basic land types are Plains/Island/Swamp/Mountain/Forest, regardless of
   * whether a qualifying land has the Basic supertype (CR 305.6). An artifact
   * creature contributes two card types; repeated values count once (205.2b).
   * Colorless contributes no color (105.2c). */
  | { amount: "distinct-count"; characteristic: "card-type" | "basic-land-type" | "color"; among: ObjectRef }
  /** Actual mana paid to cast the referenced spell, including additional
   * costs, not its printed mana cost/value (601.2f/h, 609.4b). A permanent may
   * refer to its originating spell's payment history under CR 400.7d. */
  | { amount: "mana-spent"; what: ObjectRef; quantity: "total" | "distinct-colors" }
  | { amount: "attribute-sum"; attribute: "power" | "toughness" | "mana-value"; among: ObjectRef }
  /** Multiplies a quantity inside one instruction/event; this is not a loop
   * that performs the instruction as separate events. */
  | { amount: "product"; factors: Amount[] }
  | { amount: "counter-count"; counter: CounterSpec | "any"; on: ObjectRef | PlayerRef }
  /** Number of times the originating spell's kicker/multikicker was paid,
   * retained across its move to the battlefield (CR 400.7d, 702.33). */
  | { amount: "kicker-count"; of: ObjectRef }
  | { amount: "attribute-extremum"; extremum: "maximum" | "minimum"; attribute: "power" | "toughness" | "mana-value"; among: ObjectFilter }
  | { amount: "attribute"; of: ObjectRef; attribute: "power" | "toughness" | "mana-value" }
  /** CR 700.5: the number of mana symbols of the listed color(s) among the
   * mana costs of permanents `who` controls. With two colors, each symbol
   * that is either or both colors counts once (hybrid symbols). This is not
   * mana spent, mana value, or those permanents' current colors; modifiers
   * and layer timing follow CR 700.5a. */
  | { amount: "devotion"; who: PlayerRef; colors: [Color] | [Color, Color] }
  | { amount: "life-total"; of: PlayerRef }
  /** CR 103.4: fixed when the game begins; unaffected by later life changes. */
  | { amount: "starting-life-total"; of: PlayerRef }
  /** CR 103.4 / 119.1: the format-defined total the player began the game
   * with, not their current, previous or highest life total. */
  /** Gross life gained or lost over the period, summed over every such event
   * (damage and life payments are life loss, CR 119.2–4); never a net change
   * of life total. "all-players-combined" is the printed total across players.
   * A contextual player ("they", "that player") is evaluated for that player. */
  | { amount: "life-change"; who: PlayerRef | "all-players-combined"; change: "gained" | "lost"; during: "this-turn" }
  | { amount: "twice"; of: Amount }
  | { amount: "half"; of: Amount; round: "up" | "down" }
  | { amount: "plus"; a: Amount; b: Amount }
  | { amount: "minus"; a: Amount; b: Amount }
  | { amount: "that-number" }
  | { amount: "that-much" };

/** What one earlier event actually did, after replacement and prevention
 * (CR 608.2h, 609.3, 119.3, 120.3), never the number its instruction stated
 * and never a current-state query. "this-way" is the stated event in the same
 * resolving instruction sequence; "contextual" has no printed scope (typically
 * the triggering event) and needs its antecedent event bound. A quantity is
 * the total over every matching occurrence and participant of that event;
 * omitted participants are unconstrained, not implied. */
export type EventQuantity = { scope: "contextual" | "this-way" } & (
  | { event: "life-change"; change: "gained" | "lost"; who?: PlayerRef }
  /** Excess damage follows CR 120.4a / 120.10: the part beyond lethal damage
   * (a creature), loyalty (a planeswalker) or defense (a battle). */
  | { event: "damage"; measure: "dealt" | "excess"; by?: ObjectRef; to?: ObjectRef | PlayerRef }
);

export interface SignedAmount {
  sign: 1 | -1;
  amount: Amount;
}
