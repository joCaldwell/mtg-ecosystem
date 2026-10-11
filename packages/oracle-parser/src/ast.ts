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
  occurrence?: { ordinal: number; period: "turn" };
  during?: TurnOwner;
} & (
  | { trigger: "any-of"; events: Trigger[] }
  /** CR603.1/4/10: a "while" qualification is checked only against the
   * matching event's state/snapshot, not again as this ability resolves.
   * The wrapper qualifies only this event branch. A subject-status gate
   * tests the object that participates in that occurrence; it is not a
   * freshly selected object or a current board-wide filter at resolution. */
  | { trigger: "event-gated"; event: Trigger; gate: TriggerEventGate }
  | { trigger: "enters"; what: ObjectRef; from?: ZoneRef }
  | { trigger: "leaves"; what: ObjectRef }
  | { trigger: "leaves-zone"; what: ObjectRef; from: ZoneRef }
  | { trigger: "dies"; what: ObjectRef }
  | { trigger: "enters-or-leaves"; what: ObjectRef }
  | { trigger: "enters-or-attacks"; what: ObjectRef }
  | { trigger: "enters-or-dies"; what: ObjectRef }
  | { trigger: "put-into-graveyard"; what: ObjectRef; from: Zone | ZoneRef | "anywhere"; to: ZoneRef }
  | { trigger: "cast"; who: PlayerRef; what: ObjectFilter | ObjectRef; from?: ZoneRef }
  | { trigger: "attacks"; what: ObjectRef; alone?: boolean; whom?: (PlayerRef | ObjectRef)[] }
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
  | { trigger: "deals-damage"; what: ObjectRef; combat?: boolean; to?: PlayerRef | ObjectRef | "any" }
  | { trigger: "is-dealt-damage"; what: ObjectRef }
  | { trigger: "gains-life"; who: PlayerRef }
  | { trigger: "loses-life"; who: PlayerRef }
  | { trigger: "draws"; who: PlayerRef }
  | { trigger: "discards"; who: PlayerRef; what?: ObjectRef }
  | { trigger: "sacrifices"; who: PlayerRef; what: ObjectFilter }
  | { trigger: "scries" | "surveils"; who: PlayerRef }
  | { trigger: "taps-for-mana"; what: ObjectRef });

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
        replacementChoice?: { who: PlayerRef; declined: "original" } }
    | { effect: "optional-group"; who: PlayerRef; steps: Effect[] }
    | { effect: "simultaneous"; steps: Effect[] }
    | { effect: "pay-cost"; who: PlayerRef; costs: Cost[] }
    | { effect: "play-card"; instruction: CardPlayInstruction }
    | { effect: "grant-play-permission"; permission: CardPlayInstruction; duration: Duration; durationScope?: "each-permitted-card" }
    | { effect: "grant-look-permission"; permission: LookPermission; duration: Duration }
    | { effect: "keyword-action"; action: "proliferate" | "populate" | "investigate"; who: PlayerRef }
    | { effect: "discover"; who: PlayerRef; amount: Amount }
    | { effect: "amass"; subtype: string; amount: Amount }
    | { effect: "adapt" | "monstrosity" | "bolster" | "support"; amount: Amount }
    | { effect: "explore" | "connive"; what: ObjectRef }
    | { effect: "damage"; source: ObjectRef; amount: Amount; to: DamageTarget }
    /** A no-regeneration rider modifies this destruction (CR 608.2c),
     * rather than resolving after the permanent has already been destroyed. */
    | { effect: "destroy"; what: ObjectRef; regenerationForbidden?: true }
    | { effect: "exile"; what: ObjectRef; from?: ZoneRef; who?: PlayerRef }
    /** CR 610.3: one simultaneous exile move linked to a second one-shot
     * return, not a delayed triggered ability or a duration on ordinary exile.
     * If the return event already occurred within the CR 610.3a/b window,
     * nothing moves. Returned battlefield objects default to owner control.
     * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
    | { effect: "exile-until"; what: ObjectRef[]; from?: ZoneRef; who?: PlayerRef;
        until: { event: "leaves-battlefield"; what: ObjectRef };
        return: { to: "previous-zone"; battlefieldController: "owner" } }
    | { effect: "counter"; what: ObjectRef }
    | { effect: "move-zone"; what: ObjectRef; to: ZoneRef; from?: ZoneRef; tapped?: boolean; controller?: "you" | "owner"; order?: "any" | "random"; who?: PlayerRef; entry?: BattlefieldEntryModifiers }
    | { effect: "return-to-hand"; what: ObjectRef; to: "your" | "owner"; from?: ZoneRef; who?: PlayerRef }
    | { effect: "create-token"; count: Amount; token: TokenSpec; who?: PlayerRef }
    | { effect: "pump"; what: ObjectRef; power: SignedAmount; toughness: SignedAmount }
    | { effect: "set-pt"; what: ObjectRef; power: Amount; toughness: Amount }
    | { effect: "lose-abilities"; what: ObjectRef; removal: AbilityRemoval }
    /** The printed pronoun "they" alone does not establish whether its
     * recipient is an object set or a player (CR 109.1, 113.1). Keep that
     * distinction unresolved until antecedent binding, rather than treating
     * returned creatures as players because the player grammar runs first. */
    | { effect: "gain-abilities"; what: ObjectRef | { recipient: "they" }; abilities: GainedAbility[] }
    | { effect: "player-gain-abilities"; who: PlayerRef; abilities: GainedAbility[] }
    | { effect: "gain-life" | "lose-life"; who: PlayerRef; amount: Amount }
    /** An explicit instruction to win/lose, subject to game rules and
     * replacement/prohibition effects (CR 104.2b, 104.3e), not life loss. */
    | { effect: "win-game" | "lose-game"; who: PlayerRef }
    | { effect: "tap" | "untap"; what: ObjectRef; who?: PlayerRef }
    | { effect: "untap-step-suppression"; what: ObjectRef; step: UntapStepWindow }
    | { effect: "sacrifice"; who: PlayerRef; what: ObjectRef; chosenBy?: PlayerRef }
    | { effect: "discard"; who: PlayerRef; what: ObjectRef | "hand"; random?: boolean }
    | { effect: "mill"; who: PlayerRef; amount: Amount }
    | { effect: "scry" | "surveil"; amount: Amount }
    | { effect: "put-counters"; counter: CounterSpec; count: Amount; on: ObjectRef; who?: PlayerRef }
    | { effect: "remove-counters"; counter: CounterSpec; count: Amount | "all"; from: ObjectRef }
    | { effect: "add-mana"; who: PlayerRef; mana: ManaProduction; restriction?: ManaSpendingRestriction }
    | { effect: "search"; who: PlayerRef; zones: SearchZones; for: ObjectRef }
    | { effect: "shuffle"; who: PlayerRef; library: ZoneRef }
    /** One grouped move followed by a library shuffle even when no specified
     * objects move (CR 701.24c–d), not a shuffle conditional on moving a card. */
    | { effect: "shuffle-into-library"; who: PlayerRef; what: ObjectRef[]; library: ZoneRef }
    | { effect: "gain-control"; who: PlayerRef; what: ObjectRef }
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
    | { effect: "become"; what: ObjectRef; spec: BecomeSpec }
    /** One continuous effect across layers 4–7 (CR 613.6). The affected
     * objects and direct values are fixed when it begins (611.2c/d);
     * calculations inside granted abilities remain those abilities' own
     * calculations. Ordinary grants cannot create CDAs (604.3a). */
    | { effect: "modify-characteristics"; what: ObjectRef;
        modifiers: ResolvingCharacteristicModifier[]; snapshot: "effect-created" }
    | { effect: "cant"; what: ObjectRef; action: CantAction }
    | { effect: "combat-rule"; what: ObjectRef; rule: CombatRule }
  );

export interface LookTop { top: Amount; of: ZoneRef }

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
  | ({ to: "any-target" } & TargetSelection)
  | { to: "object"; ref: ObjectRef }
  | { to: "player"; ref: PlayerRef }
  | { to: "each"; refs: (ObjectRef | PlayerRef)[] }
  | { to: "divided"; among: ObjectRef };

/** Recurring windows are checked whenever the prohibited action is proposed;
 * they are not an expiry such as "this combat" (CR 611.2c, 611.3a). */
export type ProhibitionWindow = { during: "combat" } | { during: "turn"; whose: PlayerRef };

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
  | { gained: "keyword"; keyword: KeywordInstance; protectionAttachmentException?: ProtectionAttachmentException }
  | { gained: "quoted"; ability: Ability };

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
  | { removal: "listed"; abilities: GainedAbility[] }
  | { removal: "choice"; who: PlayerRef; count: Amount; options: KeywordInstance[] };

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
  | { modifier: "colors"; operation: "set" | "add"; colors: Color[] }
  | { modifier: "base-pt"; power: Amount; toughness: Amount;
      /** A bare possessive in a direct value denotes this affected object,
       * while an explicit source reference still denotes the source. */
      valueScope?: "each-affected-object" }
  | { modifier: "abilities"; abilities: GainedAbility[] };

export type CantAction = "attack" | "block" | "attack-or-block" | "be-blocked" | "untap" | "be-countered" | "be-regenerated";

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
  | { rule: "attack-restriction"; defenders: CombatDefender[] }
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
  uses: (
    | { use: "cast"; filter: ObjectFilter; from?: ZoneRef }
    | { use: "cast-commander"; whose: PlayerRef }
    | { use: "activate"; source: ObjectFilter }
  )[];
}

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
  /** Optional payment/reveal before entry, with a declined-payment entry
   * modification. This is one replacement choice, not a tap instruction.
   * CR 614.1c, 614.12a–b. */
  | { static: "entry-choice"; what: ObjectRef; who: PlayerRef; costs: Cost[]; declined: { tapped: true } }
  | { static: "prevent-damage"; prevention: DamagePrevention }
  | { static: "prohibit"; prohibition: GameProhibition }
  | { static: "cast-limit"; limit: CastLimit }
  | { static: "play-permission"; permission: CardPlayInstruction }
  | { static: "look-permission"; permission: LookPermission }
  /** CR 903.3a / 113.6n: eligibility during deck construction, not an
   * in-game designation, characteristic, or controller-based permission. */
  | { static: "commander-eligibility"; what: Extract<ObjectRef, { ref: "self" | "this" }>; during: "deck-construction" }
  /** CR 502.3: the chooser may omit this object from the simultaneous
   * turn-based untap action; this never prohibits untapping by an effect. */
  | { static: "optional-untap"; who: PlayerRef; what: ObjectRef; step: { occurrence: "each"; owner: PlayerRef } }
  /** CR 118.9: an optional casting payment, never a resolution instruction. */
  | { static: "alternative-cost"; who: PlayerRef; spells: ObjectRef; costs: Cost[]; condition?: Condition }
  | { static: "player-abilities"; who: PlayerRef; abilities: GainedAbility[] }
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
  | { static: "untap-step-suppression"; what: ObjectRef; step: UntapStepWindow }
  | { static: "enters-with-counters"; what: ObjectRef; counter: CounterSpec; count: Amount; additional?: true; multiplier?: Amount; condition?: Condition; conditionKind?: "if" | "unless" }
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
  | { modifier: "pt"; power: SignedAmount; toughness: SignedAmount }
  | ({ modifier: "base-pt" } & (
      | { power: Amount; toughness?: Amount }
      | { power?: Amount; toughness: Amount }))
  | { modifier: "colors"; operation: "set" | "add"; colors: Color[] | ChosenValueRef<"color"> }
  | { modifier: "types"; change: TypeChange }
  | { modifier: "lose-abilities"; removal: AbilityRemoval }
  | { modifier: "abilities"; abilities: GainedAbility[] });

export type SubtypeFamily = "artifact" | "enchantment" | "land" | "planeswalker" | "spell" | "creature" | "battle" | "dungeon";
/** CR 205.1a-b / 305.7: type operations preserve the relevant family. A set
 * of basic land types invokes the rule's intrinsic mana/ability changes;
 * adding those types retains existing rules text. Setting card types still
 * retains instant/sorcery types as required by CR 205.1a. */
export interface TypeChange {
  cardTypes?: { operation: "set" | "add" | "remove"; types: string[] };
  subtypes?: { family: SubtypeFamily; operation: "set" | "add" | "remove";
    types: string[] | { all: true; basic?: boolean } }[];
  supertypes?: { operation: "add" | "remove"; types: string[] };
}

/** CR 118.7 and 601.2f: a payment adjustment does not change mana cost or
 * mana value. A caster or activation source is not an object controller. */
export type CostModificationScope =
  | { action: "cast"; spells: ObjectRef; caster?: PlayerRef; from?: ZoneRef }
  | { action: "activate"; source?: ObjectRef; activator?: PlayerRef;
      /** An ability's targets are distinct from the object bearing it. */
      abilities?: ObjectRef; keyword?: "equip" | "boast" | "cycling" | "ninjutsu" | "exhaust" | "power-up";
      loyalty?: true };

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
      event: "dies" | "enters" | "leaves-battlefield" | "put-into-zone";
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
  /** Gross life gained/lost over the period, not a net life-total change. */
  | { condition: "life-change"; who: PlayerRef; change: "gained" | "lost"; comparison: Comparison; during: "this-turn" }
  | { condition: "cast-count"; who: PlayerRef | "all-players-combined"; what: ObjectFilter; comparison: Comparison; during: "this-turn" | "last-turn"; from?: ZoneRef }
  | { condition: "control"; who: PlayerRef; what: ObjectRef }
  | { condition: "control-count"; who: PlayerRef; of: ObjectFilter; comparison: Comparison }
  /** There exists a subset of the controlled eligible objects of at least
   * minimum size satisfying all group constraints. This makes no choice and
   * need not include every controlled object. Names obey CR201.2b, including
   * objects with multiple names and nameless objects. */
  | { condition: "controls-subset"; who: PlayerRef; of: ObjectFilter; minimum: Amount; groupConstraints: GroupConstraint[] }
  | { condition: "you-do" }
  | { condition: "you-dont" }
  | { condition: "pays"; who: PlayerRef; cost: Cost[] }
  | { condition: "status"; what: ObjectRef; status: string; time?: "previous" }
  | { condition: "is-filter"; what: ObjectRef; filter: ObjectFilter; time?: "previous" }
  | { condition: "count"; of: ObjectFilter; comparison: Comparison }
  | { condition: "life-total"; who: PlayerRef; comparison: Comparison }
  | { condition: "your-turn" }
  | { condition: "not-your-turn" }
  | { condition: "remains"; what: ObjectRef; zone: ZoneRef }
  | { condition: "cards-in-hand"; who: PlayerRef; comparison: Comparison };

export type Duration =
  | { duration: "end-of-turn" }
  | { duration: "end-of-combat" }
  | { duration: "end-of-your-next-turn" }
  /** Expires as the next end step of your turn begins, not in its cleanup. */
  | { duration: "your-next-end-step" }
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
  | { ref: "from-pool"; pool: ObjectRef; count: Amount; upTo?: true; groupConstraints?: GroupConstraint[] }
  /** Distinct card allocations for search/reveal/move instructions, e.g.
   * "a creature card and/or an enchantment card". This is not a filter OR
   * under one count, nor reusable representative roles (Tragic Arrogance).
   * The chosen members' counts remain independent of the outer may choice.
   * A shared source applies to every member. CR608.2c/d, 701.23b; Kaalia's
   * M20 ruling assigns a multitype card to just one extraction slot. */
  | { ref: "card-extraction-group"; members: CardExtractionSlot[]; combine: "all" | "any-nonempty"; allocation: "distinct-objects";
      source?: { kind: "pool"; pool: ObjectRef } | { kind: "zone"; zone: ZoneRef } }
  | ({ ref: "target"; filter: ObjectFilter; count: Amount; upTo?: boolean; groupConstraints?: GroupConstraint[] } & TargetSelection)
  | ({ ref: "any-target"; count?: Amount } & TargetSelection)
  /** Symbolic event/linked referent; antecedent resolution is a later pass. */
  | { ref: "related"; filter: ObjectFilter; relation: ObjectRelation; number: "singular" | "plural" }
  /** Complement of the selected objects in the contextual or stated pool. */
  | { ref: "rest"; of?: ObjectRef }
  /** All cards in the named player's personal zone, not the zone itself. */
  | { ref: "zone-cards"; zone: "graveyard" | "hand" | "library"; owner: PlayerRef }
  /** CR 702.124e: if both commanders qualify, choice occurs as the effect applies. */
  | { ref: "commander"; owner: PlayerRef; count: Amount }
  | { ref: "each"; filter: ObjectFilter }
  | { ref: "all"; filter: ObjectFilter }
  | { ref: "filter"; filter: ObjectFilter; count: Amount; upTo?: boolean; groupConstraints?: GroupConstraint[] } // "a creature", "up to seven lands"
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
  /** The ability's single source must match this operand. A query operand
   * defines eligible sources; it does not instruct anyone to choose a source. */
  abilitySource?: ObjectRef;
  /** Directional relations of the filtered object, not its controller.
   * A query/set operand tests membership: at least one related object matches.
   * Explicit universal or numeric relation quantifiers are unsupported. */
  attachedTo?: ObjectRef | PlayerRef;
  blocking?: ObjectRef;
  blockedBy?: ObjectRef;
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
  status?: string[];                                         // tapped, attacking …
  /** A declared attacker earlier in the explicit window; a creature put
   * onto the battlefield attacking has not attacked (CR508.4). */
  combatHistory?: { action: "attacked"; during: "this-turn" | "this-combat" };
  /** Unresolved lexical "another"/"other". Its excluded antecedent is
   * contextual; it must never be assumed to mean the source by default. */
  other?: boolean;
  /** Explicit exclusion, bound where the local grammar establishes it.
   * affected-object is rebound for each object a modifier applies to. */
  otherThan?: ObjectRef | { ref: "affected-object" };
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
  own?: "you" | "opponent";
  zone?: ZoneRef;
  properties?: PropertyConstraint[];
  /** "that targets …" */
  targets?: ObjectRef | PlayerRef;
  /** "…or player": the filter unions in players ("target creature or player"). */
  orPlayer?: boolean;
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
  owner?: "your" | "their" | "its-owner" | "each-player" | "any" | "an-opponent" | "each-opponent" | "that-player"
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
  | { amount: "replacement-event-quantity" }
  | { amount: "prevention-event-quantity"; quantity: "incoming" | "prevented" }
  | { amount: "x" }
  | { amount: "chosen"; minimum: number; maximum?: Amount }
  | { amount: "count"; of: ObjectFilter }                    // "the number of Elves you control"
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
  | { amount: "life-total"; of: PlayerRef }
  | { amount: "twice"; of: Amount }
  | { amount: "half"; of: Amount; round: "up" | "down" }
  | { amount: "plus"; a: Amount; b: Amount }
  | { amount: "minus"; a: Amount; b: Amount }
  | { amount: "that-number" }
  | { amount: "that-much" };

export interface SignedAmount {
  sign: 1 | -1;
  amount: Amount;
}
