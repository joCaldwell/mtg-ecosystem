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
  | { kind: "chapter"; chapters: number[];
      /** CR 207.2c–d: the chapter ability's own ability word or flavor word. */
      abilityWord?: string; effects: Sentence[] }
  | { kind: "max-speed"; ability: Ability }
  /** CR 702.186b: "∞ — [Ability]" means "As long as this permanent is
   * harnessed, it has [Ability]" (harnessed: CR 701.64b). */
  | { kind: "infinity"; ability: Ability }
  /** CR 702.159a, 717.5: an Attraction's visit ability. It triggers whenever
   * its controller rolls to visit their Attractions (701.52, a turn-based
   * action of their precombat main phase) and the result equals a number lit
   * up on this Attraction; `effects` is its resolution text. Lit-up numbers
   * belong to each printing (717.1), not to the Oracle text, so none are
   * recorded here. A "Prize —" paragraph (702.159b) is part of the ability
   * and has no representation; those Attractions are rejected. */
  | { kind: "visit"; effects: Sentence[] }
  /** CR 123.2-123.3c, 123.7-123.8, 107.17a: one sticker of a sticker sheet
   * (type line "Stickers"). Sheets are not cards and have no characteristics
   * (123.2); each Oracle line is one sticker, preceded by its ticket cost
   * (Oracle prints the cost as that many {TK}). The owner of the object pays
   * that many ticket counters to put the sticker on it (123.3c). An ability
   * sticker makes the object it is on gain `ability` (123.7), so self
   * references in it denote that object. A power/toughness sticker sets that
   * object's power and toughness (123.8, 613.4b). Name and art stickers are
   * not part of the Oracle text. */
  | { kind: "sticker"; ticketCost: number;
      sticker: { sticker: "ability"; ability: Ability } | { sticker: "power-toughness"; power: number; toughness: number } }
  /** CR 701.43d: "You may exert [this creature] as it attacks" is an optional
   * cost to attack (508.1g). The "When you do" ability printed in the same
   * paragraph is linked to it and triggers only if exerted; `reflexive` is its
   * resolution text, where "it" is the exerted permanent. */
  | { kind: "exert-on-attack"; what: ObjectRef; reflexive?: Sentence[] }
  /** CR 603.11, 607.2a: a static ability and the triggered abilities linked
   * to it, printed as one paragraph with the static ability first. "This
   * way" in each trigger refers only to this static ability's action. */
  | { kind: "linked-triggers"; static: StaticAbility; triggers: TriggeredAbility[] }
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
  /** CR 614.12c, 607.2m: "As [this permanent] enters, choose A or B." with
   * one bulleted ability per anchor word. `who` (the permanent's controller)
   * chooses one anchor word as the permanent enters; this is a replacement
   * effect, neither a triggered ability nor a mode (700.2). For as long as
   * the permanent remains on the battlefield it has exactly the abilities
   * whose anchor word was chosen. Each ability is linked to this choice and
   * is otherwise an ordinary ability of the permanent. */
  | { kind: "anchor-words"; what: Extract<ObjectRef, { ref: "self" | "this" }>; who: PlayerRef;
      options: { word: string; ability: StaticAbility | TriggeredAbility | ActivatedAbility }[] }
  | AdditionalCostLine;

/** "Flying, first strike, ward {2}" */
export interface KeywordLine {
  kind: "keywords";
  /** CR 207.2d: a flavor word printed before the keyword ("Diana — Equip {2}"). */
  abilityWord?: string;
  keywords: KeywordInstance[];
}

/** The keyword's intrinsic class determines its Solved gate, not its
 * abbreviated printing: Equip checks activation and Ward checks triggering.
 * This does not remove/grant the keyword when the designation changes, and
 * neither gate is rechecked on resolution. Mixed-component keywords need
 * separate component gates and are not represented by this single gate. */
export interface SolvedKeywordLine {
  kind: "keywords";
  abilityWord?: string;
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
  /** Limits on the X announced while activating (CR 107.3a). */
  variableConstraints?: VariableConstraint[];
  /** "X is the number of …" as its own sentence: X is defined by the text
   * for the whole ability, costs included, and not chosen (CR 107.3a/c). */
  variableDefinitions?: VariableDefinition[];
  /** "Spend only black mana on X.": which mana may pay this ability's X. */
  paymentRestriction?: PaymentRestriction;
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

/** CR 107.3a/d/f: a printed limit on the value its controller may choose for
 * X ("X can't be 0", "X can't be greater than ..."). `maximum` is evaluated
 * when X is chosen. It limits the choice; it is not a resolving instruction. */
export type VariableConstraint = { variable: "x" } & ({ not: number } | { maximum: Amount });
export interface VariableDefinition { variable: "x"; value: Amount }

/** CR 601.2h, 602.2b: limits the mana that may pay this spell's or ability's
 * own cost while it is being paid: the whole cost, or only the X in it
 * (107.3). "colors" admits mana of any listed color; "produced-by" admits
 * mana produced by a matching source. `maximumPerColor`: at most that much
 * mana of each color may be spent on that part. Not a property of produced
 * mana (106.6). */
export interface PaymentRestriction {
  part: "x" | "total";
  mana: { mana: "colors"; colors: Color[] } | { mana: "colored" } | { mana: "produced-by"; sources: ObjectFilter };
  maximumPerColor?: number;
}

/** CR 602.5: restrictions are semantic instructions, never raw accepted text.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260819.txt */
export type ActivationRestriction =
  | { restriction: "sorcery" }
  | { restriction: "instant" }
  | { restriction: "once-each-turn" }
  | { restriction: "activation-limit"; maximum: number; period: "turn" | "object" }
  | { restriction: "timing"; turn?: TurnOwner | "activator"; phase?: PhaseName;
      before?: "declare-attackers" | "declare-blockers" | "end-step" | "end-of-combat";
      /** CR 509.1: only once blockers have been declared this combat. */
      after?: "declare-blockers" }
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
  /** Limits on an X its controller chooses for this ability (CR 107.3f). */
  variableConstraints?: VariableConstraint[];
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
  /** Limits on the X announced while casting this spell (CR 107.3a). */
  variableConstraints?: VariableConstraint[];
  /** CR 702.148a-b: this paragraph of a cleave spell has text in square
   * brackets. `effects` is the paragraph as printed, brackets' text included
   * (the spell cast for its mana cost); `cleaved` is the paragraph with all
   * bracketed text removed, which is its text while its cleave cost was paid
   * (a text-changing effect, CR 612). Exactly one of the two applies. */
  cleaved?: Sentence[];
}

/** "As an additional cost to cast this spell, sacrifice a creature." */
export interface AdditionalCostLine {
  kind: "additional-cost";
  /** CR 207.2c/207.2d: a printed ability or flavor word before the cost. */
  abilityWord?: string;
  costs: Cost[];
  /** CR 118.8b, 601.2b: "you may" — the caster announces whether to pay it. */
  optional?: true;
  /** Printed with the cost: adjusts this spell's own total cost (601.2f). */
  costModifications?: CostModification[];
  /** Limits on the X of this additional cost (CR 107.3a). */
  variableConstraints?: VariableConstraint[];
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
  /** "Firebending X, where X is ~'s power": the value of the keyword's X. */
  bindings?: { variable: "x"; value: Amount }[];
  /** Limits on the X of this keyword's cost or number ("Suspend X—{X}{R}.
   * X can't be 0."), chosen as that cost is paid (CR 107.3a/d). */
  variableConstraints?: VariableConstraint[];
  /** Protection from …, hexproof from … */
  from?: ProtectionScope[];
  /** Enchant creature, Affinity for artifacts, …walk */
  filter?: ObjectFilter;
  /** Craft with [materials] (CR 702.167a). */
  materials?: CraftMaterial[];
  /** Companion—[Condition] (CR 702.139a). */
  companion?: CompanionCondition;
  /** Partner with <name> */
  pairedWith?: string;
  enchantPlayer?: "player" | "opponent";
  prototype?: { power: Amount; toughness: Amount };
  /** Multiple independently payable kicker costs (CR 702.33). */
  additionalCosts?: Cost[][];
  gift?: "card" | "tapped-fish" | "treasure" | "food" | "octopus" | "extra-turn";
}

/** CR 702.167a-b: one description among a craft ability's [materials]. The
 * described objects are exiled as part of the activation cost, from among
 * permanents the activator controls and/or cards in their graveyard; a
 * description of cards (`cls: "card"`) admits graveyard cards only. Those
 * zones are the keyword's rule and are not repeated in `filter`. `count` is
 * exact, or a chosen number with a minimum ("one or more"). Several listed
 * descriptions are met by that many different objects. */
export interface CraftMaterial { count: Amount; filter: ObjectFilter; groupConstraints?: GroupConstraint[] }

/** CR 702.139a-b: what the owner's starting deck (after sideboard cards are
 * set aside) must fulfil before the game for this card to be their companion.
 * "each": every deck card matching `of` also matches `matches`; "contains
 * only" states that for all cards, with listed kinds as alternatives.
 * "group": the deck cards matching `of`, taken together, satisfy the
 * constraint (all different names, or one card type common to all). */
export type CompanionCondition =
  | { deck: "each"; of: ObjectFilter; matches: ObjectFilter }
  | { deck: "group"; of: ObjectFilter; constraint: GroupConstraint };

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
  /** `multiplier`: "Pay {1} for each +1/+1 counter on this creature" pays the
   * symbols that many times (a count of zero pays nothing). */
  | { cost: "mana"; symbols: string[]; multiplier?: Amount }              // ["2","G","G"] or ["X"] or ["W/P"]
  /** A quantity of mana rather than printed symbols (CR 118.1): "any amount
   * of mana", "mana equal to ...". Without `unit` any mana pays it, like a
   * generic cost of that size; with `unit` ("any amount of {R}") each unit
   * is one mana that symbol could pay. */
  | { cost: "mana-amount"; amount: Amount; unit?: string }
  /** "Pay its mana cost": the mana cost printed on that object (CR 202.1),
   * paid as mana (CR 118.1). The object is kept symbolic like any other
   * reference; one without a mana cost can't be paid this way (CR 118.6). */
  | { cost: "mana-cost-of"; of: ObjectRef }
  | { cost: "tap-self" }                             // {T}
  | { cost: "untap-self" }                           // {Q}
  | { cost: "energy"; amount: Amount }               // {E}{E}, Pay X {E}
  /** `multiplier`: "Pay 3 life for each velocity counter" pays `amount` that many times. */
  | { cost: "pay-life"; amount: Amount; multiplier?: Amount }
  /** CR 118.1: the payer has the stated player(s) gain this much life. With
   * "an opponent" the payer chooses that opponent while paying. */
  | { cost: "have-gain-life"; who: PlayerRef; amount: Amount }
  | { cost: "sacrifice"; what: ObjectRef }
  | { cost: "discard"; what: ObjectRef | "hand"; random?: boolean }
  | { cost: "reveal"; what: ObjectRef; from?: ZoneRef }
  | { cost: "choice"; options: Cost[][] }
  | { cost: "exile"; what: ObjectRef; from?: ZoneRef }
  | { cost: "tap-objects"; what: ObjectRef }         // "Tap two untapped Allies you control"
  | { cost: "untap-objects"; what: ObjectRef }
  /** CR 701.3d: the payer moves the attached permanent off what it is
   * attached to; it stays on the battlefield attached to nothing. Unpayable
   * while that permanent is not attached (CR 118.3). */
  | { cost: "unattach"; what: ObjectRef }
  | { cost: "exert"; what: ObjectRef }
  | { cost: "mill"; amount: Amount }
  | { cost: "return"; what: ObjectRef; to: "hand"; owner: "your" | "owner"; from?: ZoneRef }
  | { cost: "remove-counters"; counter: CounterSpec | "any"; count: Amount | "all"; from: ObjectRef; distributed?: true }
  | { cost: "put-counters"; counter: CounterSpec; count: Amount; on: ObjectRef }
  /** "Put a card from your hand on top of your library", "Put a card exiled
   * with this artifact into its owner's graveyard": the same zone change as
   * the `move-zone` effect, paid as a cost (CR 118.1, 400.3, 401.4). */
  | { cost: "move-zone"; what: ObjectRef; to: ZoneRef }
  | { cost: "collect-evidence"; amount: Amount }
  | { cost: "waterbend"; symbols: string[] }
  | { cost: "forage" }
  /** CR 701.4a: reveal a [quality] card from hand or choose a [quality] permanent
   * you control. `exile`: the beheld card or permanent is exiled as part of the cost. */
  | { cost: "behold"; quality: ObjectFilter; count: Amount; exile?: true }
  | { cost: "blight"; amount: Amount }
  | { cost: "get-counters"; counter: CounterSpec; count: Amount };

// ---------------------------------------------------------------------------
// Triggers
// ---------------------------------------------------------------------------

/** Omitted grouping means each matching occurrence; one-or-more means once
 * for the matching group in an event (CR 603.2c, 700.1). */
export type Trigger = {
  grouping?: "one-or-more";
  /** With grouping: the matching group in the event must have this size
   * ("two or more creatures you control attack"); omitted is at least one. */
  groupSize?: Comparison;
  controller?: PlayerRef;
  /** `per: "player"` ("their second spell each turn") counts separately for
   * each player performing the event; otherwise the matching events of the
   * period are counted together. */
  occurrence?: { ordinal: number; period: "turn"; per?: "player" };
  /** "a spell other than your first spell each turn": every matching event of
   * the period except the Nth one (counted as `occurrence`, same `per`). */
  notOccurrence?: { ordinal: number; period: "turn"; per?: "player" };
  during?: TurnOwner;
  /** "during combat", "during your main phase": the event happens within
   * such a phase or step (CR 500.1); whose limits it to that player's turn.
   * Independent of, and combinable with, the turn restriction `during`. */
  inPhase?: { phase: Exclude<PhaseName, "turn">; whose?: "your" };
  /** "a spell or ability an opponent controls causes you to …": only
   * occurrences brought about by an effect of a matching spell or ability
   * (CR 609.1), never by a cost payment or a game rule. */
  causedBy?: ObjectFilter;
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
  | { trigger: "enters"; what: ObjectRef; from?: ZoneRef; notFrom?: ZoneRef; untapped?: true; tapped?: true; transformed?: true }
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
  /** fromAnyOf ("from your library and/or your graveyard"): the move is from
   * any of these zones, and cards moved from both at once are one occurrence
   * when grouped (CR 603.2c); `from` is then omitted. */
  | { trigger: "put-into-zone"; what: ObjectRef; from?: ZoneRef | "anywhere"; fromAnyOf?: ZoneRef[]; to: ZoneRef }
  /** notFrom ("from anywhere other than their hand") excludes one origin
   * zone of the cast (CR 601.2a); it does not list the remaining zones. */
  /** usingManaFrom ("using mana produced by ~"): at least one of the mana
   * spent to cast it was produced by that source (CR 601.2g, 106.4). */
  | { trigger: "cast"; who: PlayerRef; what: ObjectFilter | ObjectRef; from?: ZoneRef; notFrom?: ZoneRef; usingManaFrom?: ObjectRef }
  /** CR 701.18a/b: playing a land is the special action, not a land entering
   * by an effect; playing a card is that land play or casting it as a spell.
   * from is the zone the card was played from, not its zone afterwards. */
  | { trigger: "plays"; who: PlayerRef; what: ObjectFilter | ObjectRef; from?: ZoneRef }
  /** alongside: the same declaration also contains this many other attackers
   * matching the filter, besides the subject (CR 508.1, 508.3a). */
  | { trigger: "attacks"; what: ObjectRef; alone?: boolean; whom?: (PlayerRef | ObjectRef)[];
      alongside?: { filter: ObjectFilter; count: Comparison } }
  /** CR 509.3g: triggers when no creatures are declared as blockers for the
   * attacking creature, even if it was never declared as an attacker. It is
   * not an attack event (508.1) and does not trigger when a blocked
   * creature's blockers all leave combat. */
  | { trigger: "attacks-and-isnt-blocked"; what: ObjectRef; whom?: (PlayerRef | ObjectRef)[] }
  | { trigger: "you-attack" }
  /** CR 508.3b: "Whenever [a player] is attacked" triggers once when one or
   * more creatures are declared as attackers attacking that player; a
   * creature put onto the battlefield attacking does not trigger it. */
  | { trigger: "is-attacked"; who: PlayerRef }
  /** CR 508.3c–e: "with a creature" triggers for each matching attacker;
   * "with one or more" triggers once per declaration (or attacked player).
   * count measures the qualifying attackers in that single declaration. */
  | { trigger: "player-attacks"; who: PlayerRef; with?: ObjectFilter; count?: Comparison;
      whom?: PlayerRef; per: "attacker" | "declaration";
      /** whomGrouping ("attacks one or more of your opponents", "attacks you
       * and/or one or more planeswalkers you control"): attacking any of the
       * stated defenders in one declaration is one occurrence (CR 508.3e).
       * whomPermanents are the planeswalkers or battles among them. Without
       * whomGrouping ("attacks a planeswalker you control", "attack a player or
       * planeswalker") each defender attacked is its own occurrence (CR 508.3e). */
      whomPermanents?: ObjectRef; whomGrouping?: "one-or-more";
      /** "attacks with creatures with total power 6 or greater": the powers of
       * all creatures declared as attackers by that player in the one
       * declaration (508.1), added together, satisfy the comparison. */
      totalPower?: Comparison }
  /** blockingGrouping / byGrouping ("blocks one or more black creatures",
   * "becomes blocked by two or more creatures"): the matching creatures on
   * the other side of the block are one occurrence, not one per creature
   * (contrast CR 509.3b/d); the size is the number of them that must block or
   * be blocked by `what` when blockers are declared (CR 509.3e). */
  | { trigger: "blocks"; what: ObjectRef; orBecomesBlocked?: boolean; blocking?: ObjectRef;
      blockingGrouping?: "one-or-more"; blockingSize?: Comparison }
  | { trigger: "becomes-blocked"; what: ObjectRef; by?: ObjectRef; byGrouping?: "one-or-more"; bySize?: Comparison }
  | { trigger: "becomes-target"; what: ObjectRef; of: ObjectFilter }
  /** CR 115.1, 601.2c: the player becomes a target of a matching spell or
   * ability; the player counterpart of becomes-target. */
  | { trigger: "player-becomes-target"; who: PlayerRef; of: ObjectFilter }
  /** CR 701.3a: the Aura, Equipment or Fortification is put onto `to`, on
   * entering attached or by moving. Unattached: it stops being attached to
   * `from`, including when either leaves the battlefield (603.10c looks back). */
  | { trigger: "becomes-attached"; what: ObjectRef; to: ObjectRef }
  | { trigger: "becomes-unattached"; what: ObjectRef; from: ObjectRef }
  | { trigger: "becomes-tapped" | "becomes-untapped"; what: ObjectRef }
  /** crewed: a crew ability of this Vehicle resolves (CR 702.122e); saddled:
   * it gets the saddled designation (702.171b); plotted: this card in exile
   * becomes a plotted card (702.170a/c). */
  | { trigger: "becomes-status"; what: ObjectRef; status: "monstrous" | "renowned" | "crewed" | "saddled" | "plotted" }
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
  /** The permanent completes the named action: explores (CR 701.44b),
   * connives (701.50a/e), fights (701.14a/b), evolves (702.100b: its evolve
   * ability puts counters on it), trains (702.149c), phases in or out
   * (702.26b/c; no zone change, 702.26d). */
  | { trigger: "explores" | "connives" | "fights" | "evolves" | "trains" | "phases-in" | "phases-out"; what: ObjectRef }
  /** `what` acts on `recipient`: it is tapped to pay that Vehicle's crew cost
   * (CR 702.122b) or that permanent's saddle cost (702.171c), is the creature
   * whose enlist cost taps recipient (702.154c), or is the source of a
   * resolving mentor ability targeting recipient (702.134c). */
  | { trigger: "crews" | "saddles" | "enlists" | "mentors"; what: ObjectRef; recipient: ObjectRef }
  /** into.filter ("transforms into a Phyrexian"): the permanent, as it is
   * after transforming (CR 701.27), matches the description. */
  | { trigger: "transforms"; what: ObjectRef; into?: { face: "source-face" } | { filter: ObjectFilter } }
  | { trigger: "phase"; phase: PhaseName; whose: TurnOwner | PlayerRef }
  /** CR 731.1a: "day becomes night" / "night becomes day" — the game loses
   * the other designation and gains `to`. Becoming day or night from
   * neither is not this event. */
  | { trigger: "day-night-changes"; to: "day" | "night" }
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
  /** recipients ("to one or more players"): every matching recipient damaged
   * in the event belongs to one occurrence (CR 603.2c), instead of one
   * occurrence per damaged recipient. */
  /** amount ("deals 6 or more damage"): the amount of damage dealt in the
   * one event satisfies the comparison. */
  | { trigger: "deals-damage"; what: ObjectRef; combat?: boolean; to?: PlayerRef | ObjectRef | "any"; recipients?: "one-or-more";
      amount?: Comparison }
  /** amount ("is dealt 3 or more damage"): the damage dealt in the event
   * satisfies the comparison. excess ("is dealt excess damage"): CR 120.10,
   * a creature, planeswalker or battle was dealt more than lethal damage
   * (creature), loyalty (planeswalker) or defense (battle). */
  | { trigger: "is-dealt-damage"; what: ObjectRef; combat?: boolean; amount?: Comparison; excess?: true }
  /** The printed event names only the damaged player and no source, so the
   * occurrence is the player being dealt damage (CR 120.3a/b, 603.2c); it
   * is not rewritten as a per-source deals-damage event. amount ("is dealt
   * 3 or more damage"): the total dealt to the player in the event; with
   * singleSource ("by a single source"): one source dealt that much. */
  | { trigger: "player-dealt-damage"; who: PlayerRef; combat?: boolean; amount?: Comparison; singleSource?: true }
  | { trigger: "gains-life"; who: PlayerRef }
  | { trigger: "loses-life"; who: PlayerRef }
  /** except: every draw other than the first card that player draws in each
   * of their own draw steps (same exception as ReplacementEvent "draw"). */
  | { trigger: "draws"; who: PlayerRef; except?: "first-in-each-draw-step" }
  | { trigger: "discards"; who: PlayerRef; what?: ObjectRef }
  | { trigger: "sacrifices"; who: PlayerRef; what: ObjectFilter | ObjectRef }
  /** The player stops controlling the object (CR 603.10d: looks back in time). */
  | { trigger: "loses-control"; who: PlayerRef; what: ObjectRef }
  | { trigger: "scries" | "surveils"; who: PlayerRef }
  /** The player performs the named keyword action or special action:
   * foretelling a card (CR 702.143c), discovering (701.57b), manifesting
   * dread (701.62b), waterbending (701.67c), opening an Attraction (701.51c),
   * an Attraction of theirs being visited (701.52a), giving a gift (702.174c),
   * solving a Case (719.3a), choosing a Ring-bearer (701.54a). */
  | { trigger: "foretells" | "discovers" | "manifests-dread" | "waterbends" | "opens-attraction" | "visits-attraction"
      | "gives-gift" | "solves-case" | "chooses-ring-bearer"; who: PlayerRef }
  /** The player turns a permanent face up (CR 116.2b, 701.40b), as opposed
   * to a permanent being turned face up by anything. */
  | { trigger: "turns-face-up"; who: PlayerRef; what: ObjectRef }
  /** CR 603.11, 607.2a: "Whenever you reveal a [card] this way": `who`
   * reveals a card matching `what` by the linked static ability printed
   * first in the same paragraph; no other reveal matches. */
  | { trigger: "reveals-linked"; who: PlayerRef; what: ObjectFilter }
  /** CR 123.3: the player puts a sticker of this kind ("any": of any kind)
   * on an object matching `on`. */
  | { trigger: "puts-sticker"; who: PlayerRef; sticker: StickerKind; on: ObjectRef }
  /** CR 119.4: the player pays life, as a cost or by an effect. */
  | { trigger: "pays-life"; who: PlayerRef }
  /** CR 714.2e: the Saga's chapter ability with its final chapter number
   * triggers (put on the stack) or resolves. */
  | { trigger: "final-chapter-ability"; of: ObjectRef; event: "triggers" | "resolves" }
  /** The upkeep cost of `what`'s own cumulative upkeep (CR 702.24a) or echo
   * (702.30a) ability is paid (`paid: true`) or declined (`paid: false`, the
   * player "doesn't pay"). `who` is the paying player; absent in the passive
   * form "<permanent>'s cumulative upkeep is paid". */
  | { trigger: "pays-upkeep-cost"; keyword: "cumulative-upkeep" | "echo"; what: ObjectRef; who?: PlayerRef; paid: boolean }
  /** CR 701.38a: every player has finished voting in a vote; `votedFor`
   * player filters and vote counts then read that vote's results. */
  | { trigger: "players-finish-voting" }
  /** The player completes the named action or event: proliferating (CR
   * 701.34a), completing a dungeon (309.7), searching or shuffling their own
   * library (701.23a, 701.24a/b), losing the game (104.3), becoming the
   * monarch (725.3), being tempted by the Ring (701.54d), collecting
   * evidence (701.59a), foraging (701.61a), or investigating (701.16a). */
  | { trigger: "proliferates" | "investigates" | "completes-dungeon" | "searches-library" | "shuffles-library" | "loses-game"
      | "becomes-monarch" | "ring-tempts" | "collects-evidence" | "forages"; who: PlayerRef }
  /** CR 603.10d: the player loses control of the object (or, in gains-control,
   * gains control of it, optionally from a previous controller named `from`);
   * these abilities look back in time. */
  | { trigger: "loses-control"; who: PlayerRef; what: ObjectRef }
  | { trigger: "gains-control"; who: PlayerRef; what: ObjectRef; from?: PlayerRef }
  /** CR 701.26a: the player taps a matching permanent, as a cost or by an
   * effect; tapping for mana is the narrower taps-for-mana event. */
  | { trigger: "taps"; who: PlayerRef; what: ObjectRef }
  /** CR 701.6a: a matching spell or ability counters a matching spell or
   * ability, the event's object. */
  | { trigger: "counters"; by: ObjectFilter; what: ObjectRef }
  /** CR 701.30: the player clashes; won restricts it to clashes that player
   * wins (701.30d, "win a clash", "clash and win"). */
  | { trigger: "clashes"; who: PlayerRef; won?: true }
  /** CR 705.2: the player who flipped the coin wins or loses that flip.
   * Flips that only produce heads or tails have no winner or loser. */
  | { trigger: "coin-flip"; who: PlayerRef; result: "win" | "lose" }
  /** CR 701.7a, 111.1: the player creates a matching token, the event's
   * object; one occurrence per token unless grouped. */
  | { trigger: "creates-token"; who: PlayerRef; what: ObjectRef }
  /** CR 701.17a: a matching card is milled. who omitted ("cards are
   * milled"): by any player, and a one-or-more group is every matching card
   * milled in that event, whoever milled it. */
  | { trigger: "mills"; who?: PlayerRef; what: ObjectRef }
  /** CR 302.2: the player puts a matching object onto the battlefield (the
   * player is the one performing the put, not merely the new controller). */
  | { trigger: "puts-onto-battlefield"; who: PlayerRef; what: ObjectRef; grouping?: "one-or-more" }
  /** CR 701.43a: the player exerts a matching permanent, the event's object. */
  | { trigger: "exerts"; who: PlayerRef; what: ObjectRef }
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
  /** CR 709.5i: the permanent has one unlocked designation and gets the
   * other, or has neither and gains both. what is the Room. */
  | { trigger: "fully-unlocks"; who: PlayerRef; what: ObjectFilter }
  /** Contraption rules are outside the Comprehensive Rules (CR 701.45a
   * defers to the Unstable FAQ); this names the printed crank event only. */
  | { trigger: "cranks"; who: PlayerRef; what: ObjectRef }
  /** CR 106.12/106.12a: a mana ability with {T} in its cost resolves and
   * produces mana. who, if printed, is the player who activated it. producing
   * ("tapped for {C}"): the mana produced includes that type (106.12a). */
  | { trigger: "taps-for-mana"; what: ObjectRef; who?: PlayerRef; producing?: "W" | "U" | "B" | "R" | "G" | "C" });

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

/** "event-player": "during their turn" in a trigger whose event is performed
 * by one other player — the turn of the player of that very occurrence. */
export type TurnOwner = "your" | "each-player" | "each-opponent" | "that-player" | "active-player" | "opponent" | "event-player";
/** A particular turn occurrence. Absent next-turn owner means the next turn
 * in the game's schedule, not the ability controller's next turn. A `that`
 * reference requires a contextual turn-event binding, never a player/object
 * antecedent or an invented most-recent-added-turn heuristic. */
export type TurnRef =
  | { turn: "current" }
  | { turn: "next"; whose?: PlayerRef }
  /** The first turn of the game; with `whose`, the first turn that player
   * takes. "The first upkeep" is the upkeep of the game's first turn, "your
   * first upkeep" that of your first turn (CR 103.8, 500.1). */
  | { turn: "first"; whose?: PlayerRef }
  | { turn: "that"; scope: "contextual" };

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

/** One sentence of resolution text: connected effects plus an optional leading condition. */
/** The moment a spell is cast (CR 601.2) or an ability activated (602.2). */
export type AnnouncementTime = "casting" | "activation";

export interface Sentence {
  /** "If you control a Dragon, …" / "… unless its controller pays {2}." */
  condition?: Condition;
  conditionKind?: "if" | "unless";
  /** "Return target creature to its owner's hand if its power is 2 or less":
   * the "if" clause follows the instruction it governs, so its references
   * may name that instruction's objects (CR 608.2c). */
  conditionPosition?: "after";
  /** Effects joined by "," / "then" / "and" — executed in order. */
  steps: Effect[];
  /** A following "Otherwise" can test the condition or the actual outcome
   * of the sole optional action. action-not-performed also covers failing
   * the condition, declining the action, or being unable to carry it out. */
  otherwise?: { on: "condition-false" | "condition-true" | "action-not-performed"; steps: Effect[] };
  /** Explicit definitions in this instruction, e.g. "where X is its power".
   * `asOf`: "... as you cast this spell" / "as you activate this ability"
   * fixes the value while the spell is cast or the ability activated
   * (CR 601.2, 602.2), never again on resolution. Without `asOf` the value
   * is determined once, as the instruction resolves, also for a lasting
   * effect it creates (CR 608.2h); "as this ability resolves" states that. */
  bindings?: { variable: "x"; value: Amount; asOf?: AnnouncementTime }[];
  /** This sentence follows a conditional sentence and refers back by pronoun
   * or demonstrative, possibly to what that sentence acted on or introduced:
   * "If you do, return that card to the battlefield.
   * It gains haste."; "If this spell was kicked, untap all Forests put onto
   * the battlefield this way. They become 3/3 creatures." Whether the
   * printed condition also governs this sentence depends on the pronoun's
   * antecedent (CR 608.2c), which the parser does not decide. It must be
   * resolved before execution; treating the sentence as unconditional is
   * not safe (Kheru Lich Lord would exile a card it never returned). */
  afterConditional?: "scope-unresolved";
}

interface EffectBase {
  /** "You may …" */
  optional?: true;
  /** "[Player] may …": the player other than you who decides whether this
   * optional instruction is performed, and who then performs it (CR 608.2d,
   * 118.12). Only set together with `optional`; absent means you decide. A
   * quantified player ("each opponent may") is every such player deciding
   * separately, in APNAP order, for their own action (CR 101.4). In the
   * causative "[player] may have …" the decider only decides: the
   * instruction keeps its own printed actor or damage source. */
  decidedBy?: PlayerRef;
  /** "Starting with you, each player …": the players make their choices
   * for this instruction one after another in turn order, beginning with
   * this player instead of the active player (CR 101.4, 101.4b); only set
   * on an instruction carried out by each of several players. */
  startingWith?: PlayerRef;
  /** "… for each Elf you control." */
  forEach?: ObjectFilter;
  /** "… for each creature blocking it beyond the first": the first
   * matching object is not counted (so one match counts zero). */
  forEachBeyond?: 1;
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
        sequence: { phase: "beginning" | "main" | "combat"; count: Amount }[] }
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
     * dedicated binding scope; they are not prior global event results.
     * Sentences after the loop's own sentence are outside it and done once.
     * When they refer back, it is in the plural to what every member's
     * single object produced together ("Those tokens gain haste."). */
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
    /** "Target player loses all poison counters": every counter of that kind
     * (counter "any": of every kind) is removed from the player (CR 122.1). */
    | { effect: "lose-counters"; who: PlayerRef; counter: CounterSpec | "any"; count: "all" }
    | { effect: "get-emblem"; who: PlayerRef; abilities: GainedAbility[] }
    /** Target choices are announced under CR 601.2c, not during resolution.
     * `asOf`: the printed "as you activate this ability" / "as you cast this
     * spell" evaluates the target's qualification only then; it is not
     * measured again when legality is rechecked on resolution (608.2b). */
    | { effect: "declare-target"; what: ObjectRef | PlayerRef; asOf?: AnnouncementTime }
    | { effect: "choose-object"; who: PlayerRef; what: ObjectRef | PlayerRef }
    /** "For each player, choose friend or foe": as this instruction resolves,
     * `who` labels every player of `players` (you included) either friend or
     * foe (CR 608.2d); it is neither a target nor a vote. Later "each friend"
     * / "each foe" of the same ability are the players so labeled
     * (PlayerFilter.friendOrFoe). */
    | { effect: "choose-friend-or-foe"; who: PlayerRef; players: PlayerRef }
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
    /** CR 722.3a-b: the permanent gains or loses the prepared designation.
     * Only a permanent with a prepare spell that isn't already prepared can
     * gain it; gaining it creates the castable copy in exile (722.3c). */
    | { effect: "prepare" | "unprepare"; what: ObjectRef }
    /** An effect gives the designation directly: a plotted (CR 702.170c-d)
     * or foretold (702.143d, no foretell cost unless one is given) card in
     * exile, or a saddled permanent until end of turn (702.171b). "Harness
     * [this permanent]" (701.64a) makes a permanent that isn't harnessed
     * harnessed; the designation lasts until it leaves the battlefield. */
    | { effect: "become-status"; what: ObjectRef; status: "plotted" | "foretold" | "saddled" | "harnessed" }
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
    /** CR 603.12. `event-result` is "when [something happens] this way":
     * the ability triggers once if the earlier instruction of this resolution
     * did the event to that many matching objects. `of` carries the
     * event-result relation, including its printed actor. */
    | { effect: "create-reflexive-trigger"; event: { event: "preceding-action"; who: PlayerRef; performed: boolean }
          /** "When you pay this cost one or more times": the preceding repeated pay-cost was paid at least once. */
          | { event: "repeated-payment"; who: PlayerRef }
          | { event: "event-result"; of: ObjectFilter; comparison: Comparison }
          /** "When it connives this way" (CR 701.50a, 603.12): the earlier connive instruction of this resolution was carried out by that object. */
          | { event: "connived"; what: ObjectRef }
          /** "When it regenerates this way" (CR 701.19a, 603.12): the earlier regenerate instruction's shield was applied to that permanent. */
          | { event: "regenerated"; what: ObjectRef }
          /** "When damage [from a red source] is prevented this way" (CR
           * 615.5, 603.12): triggers each time the enclosing prevention
           * prevents matching damage; "that much" is the prevented amount. */
          | { event: "damage-prevented"; source?: ObjectFilter }; effects: Sentence[] }
    | { effect: "create-replacement"; replacement: ReplacementInstruction }
    /** CR 614.15 / 616.1a: modifies this spell/ability's own instruction and
     * applies before other replacements. Alternative kicker targets are
     * announced from the applicable branch, not from both (601.2c, 702.33g). */
    | { effect: "self-replacement";
        /** One replaced instruction, or the whole preceding sentence when it
         * has several instructions, its own condition, or a definition of X. */
        original: Effect | InstructionSequence; condition: Condition;
        /** Instructions of one unconditional sentence, or a whole sentence
         * that carries its own condition or definition of X. */
        instead: Effect[] | InstructionSequence;
        /** "alternative-by-cost": the same announcement rule for another
         * additional cost chosen while casting (601.2c; bargain 702.166d,
         * gift 702.174m, teamwork 702.194c, the spell's own optional
         * additional cost). */
        targetPolicy: "retain-original" | "alternative-by-kicker" | "alternative-by-cost";
        /** Declining an optional replacement performs the original effect;
         * this is not an optional action after replacing the original. */
        replacementChoice?: { who: PlayerRef; declined: "original" };
        /** CR 614.15: the replacement was printed as its own paragraph after
         * the replaced ability; this is that paragraph's ability word, if
         * any (207.2c: no rules meaning). The block spans both paragraphs. */
        abilityWord?: string }
    | { effect: "optional-group"; who: PlayerRef; steps: Effect[] }
    /** "Repeat this process": the instructions of this ability that "this
     * process" names are carried out again, in order, as they are printed
     * (CR 608.2c); which earlier instructions those are is contextual and
     * must be bound, never assumed to be the whole ability. Without `times`
     * the repetition includes this instruction and the condition governing
     * it, so it can recur. A number ("once", "X more times") is exactly that
     * many further repetitions, which do not repeat again. "any-number"
     * ("you may repeat this process any number of times"): after each
     * repetition the deciding player chooses whether to repeat once more. */
    | { effect: "repeat-process"; process: "contextual"; times?: Amount | "any-number" }
    /** CR 608.2d: exactly one listed instruction is performed, chosen by who
     * while the effect is applied. It is not a mode announced with the spell
     * or ability (700.2), and an illegal or impossible option can't be
     * chosen. A subject, object, or target shared by the printed alternatives
     * is the same reference (and target selection) in every option. */
    | { effect: "choice"; who: PlayerRef; options: Effect[] }
    /** CR 701.55a-d: `who` faces a villainous choice: they choose one of the
     * two options, then all actions of the chosen option are performed, in
     * order. Unlike `choice`, an illegal or impossible option may be chosen;
     * as much of it as possible is done (701.55b). Several facing players go
     * through the whole process one at a time in APNAP order (701.55d). In
     * the options, "that player" / "they" stay symbolic for the facing player. */
    | { effect: "villainous-choice"; who: PlayerRef; options: [Effect[], Effect[]] }
    | { effect: "simultaneous"; steps: Effect[] }
    /** `times` repeats the payment: "any number of times" or "up to N times"
     * (the player chooses how many; zero is allowed). A following "When you
     * pay this cost one or more times" reflexive trigger refers to it, and
     * "that many" there is the number of times it was paid. */
    | { effect: "pay-cost"; who: PlayerRef; costs: Cost[]; times?: "any-number" | { upTo: Amount } }
    | { effect: "play-card"; instruction: CardPlayInstruction }
    | { effect: "grant-play-permission"; permission: CardPlayInstruction; duration: Duration; durationScope?: "each-permitted-card" }
    | { effect: "grant-look-permission"; permission: LookPermission; duration: Duration; durationScope?: "each-permitted-card" }
    /** CR 305.2: for the duration, raises the number of lands the player can
     * play in a turn. "May" describes the later land plays; creating this
     * continuous effect is mandatory (611.2a). See LandPlayAllowance. */
    | { effect: "additional-land-plays"; allowance: LandPlayAllowance; duration: Duration }
    /** `times` is "twice" / "N times": the action happens that many times in
     * sequence, as separate events (not one scaled event). */
    | { effect: "keyword-action"; action: "proliferate" | "populate" | "investigate"; who: PlayerRef; times?: Amount }
    /** Designations and player-level keyword actions with no further
     * parameters: CR 725.2 (become the monarch), 726.2 (take the initiative),
     * 701.54a (the Ring tempts), 701.48a, 701.61a, 701.56a, 701.51b, 701.31b (planeswalk). */
    | { effect: "become-monarch" | "take-initiative" | "ring-tempts" | "learn" | "forage" | "time-travel" | "open-attraction" | "planeswalk"; who: PlayerRef }
    /** Contraption rules are outside the CR (see the "cranks" trigger): the
     * printed "<Contraption> assembles a Contraption / a number of
     * Contraptions"; `by` is the assembling Contraption as printed. */
    | { effect: "assemble-contraption"; by: ObjectRef; count: Amount }
    /** CR 311.7, 901.9b: "chaos ensues" triggers the chaos abilities of the planar controller's plane. */
    | { effect: "chaos-ensues" }
    /** CR 701.4a: reveal a [quality] card from your hand or choose a [quality]
     * permanent you control on the battlefield. */
    | { effect: "behold"; who: PlayerRef; quality: ObjectFilter; count: Amount }
    /** CR 701.59a: exile cards from your graveyard with total mana value N or
     * greater (the effect form of the collect-evidence cost). */
    | { effect: "collect-evidence"; who: PlayerRef; amount: Amount }
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
     * ability that goaded it. The goading player is the controller. A stated
     * `duration` ("goaded for the rest of the game") replaces that default. */
    | { effect: "goad"; what: ObjectRef; duration?: Duration }
    /** CR 709.5f: the acting player chooses a locked door (half) of the Room
     * and it gets the matching unlocked designation. */
    | { effect: "unlock-door"; what: ObjectRef }
    /** CR 709.5f–g: "lock or unlock a door of": the acting player chooses
     * either to unlock a locked half or to lock an unlocked half of the
     * Room, and which half, as the instruction resolves. */
    | { effect: "lock-or-unlock-door"; what: ObjectRef }
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
    /** A plural `source` ("two target creatures each deal damage equal to
     * their power") deals damage once per source object, each its own
     * amount: `it` in `amount` is that source (CR 120.3, 608.2c).
     * `unpreventable`: the printed rider "the damage can't be prevented"
     * on this instruction's own damage (CR 608.2c, 615.12). Prevention
     * effects still apply to it but prevent none of it; it is not a rule
     * about other damage dealt this turn. */
    | { effect: "damage"; source: ObjectRef; amount: Amount; to: DamageTarget; unpreventable?: true;
        /** CR 120.4a: the part of this damage beyond lethal damage (a
         * creature) or loyalty/defense that would be dealt to the recipient
         * permanent is dealt to this player instead. */
        excessTo?: PlayerRef }
    /** A no-regeneration rider modifies this destruction (CR 608.2c),
     * rather than resolving after the permanent has already been destroyed. */
    | { effect: "destroy"; what: ObjectRef; regenerationForbidden?: true }
    /** CR 406.3: "face down" is information-bearing; a face-down exiled card
     * has no characteristics and cannot be examined except as instructed.
     * `counters` are on the card as it arrives in exile (CR 702.62a's
     * "exile it with N time counters on it"), not a later put-counters step. */
    | { effect: "exile"; what: ObjectRef; from?: ZoneRef; who?: PlayerRef; faceDown?: true; counters?: { counter: CounterSpec; count: Amount }[]; pile?: true }
    /** CR 701.24a: randomize the order of the face-down pile just exiled
     * ("exile ... in a face-down pile, shuffle that pile"). */
    | { effect: "shuffle-pile"; who: PlayerRef }
    /** CR 700.3: `who` groups all of `what` into the two piles. Every object
     * goes into exactly one pile (700.3a), either pile may be empty (700.3d)
     * and nothing changes zones (700.3c). A printed facing says which pile's
     * cards the other players can see. */
    | { effect: "separate-piles"; who: PlayerRef; what: ObjectRef; piles: [PileSpec, PileSpec] }
    /** `who` picks one pile of the preceding separation as this is carried
     * out (CR 608.2d); never a target. Later "that pile" is the picked one. */
    | { effect: "choose-pile"; who: PlayerRef }
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
    /** CR 106.4: all mana in the player's mana pool is lost now, as when it
     * empties at the end of a step or phase. */
    | { effect: "lose-unspent-mana"; who: PlayerRef }
    /** CR 119.5: the player gains or loses the necessary amount of life to
     * end up with this total. For several players each amount is evaluated
     * for that player, and all are determined before any total changes. */
    | { effect: "set-life-total"; who: PlayerRef; amount: Amount }
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
    /** CR 701.29a: `who` looks at the top `amount` cards of an opponent's library
     * (the opponent is chosen as it is performed) and rearranges them. */
    | { effect: "fateseal"; who: PlayerRef; amount: Amount }
    /** `upTo`: the player chooses any number from zero to `count`. `maxTotal`: "This
     * ability can't cause the total number of [counters] on [it] to be greater
     * than N" caps how many are put so the object has no more than N of them. */
    | { effect: "put-counters"; counter: CounterSpec; count: Amount; on: ObjectRef; who?: PlayerRef; upTo?: true; maxTotal?: Amount }
    /** CR 123.3-123.3c: `who` chooses one sticker of this kind ("any": of
     * any kind) among the stickers they have access to that are on no object
     * they own, pays its ticket cost, and puts it on `on`. A sticker whose
     * cost they can't pay can't be chosen; nothing is put on an object they
     * don't own (123.3b). */
    | { effect: "put-sticker"; who: PlayerRef; sticker: StickerKind; on: ObjectRef }
    /** counter "any": the removing player chooses among the counters present,
     * of any kinds. upTo permits any smaller number, including zero. */
    | { effect: "remove-counters"; counter: CounterSpec | "any"; count: Amount | "all"; from: ObjectRef; upTo?: true }
    /** CR 701.10e: give the player or permanent as many counters of that kind
     * as it already has. "each-kind" doubles every kind separately. */
    | { effect: "double-counters"; on: ObjectRef; counter: CounterSpec | "each-kind" }
    /** "Put its counters on …" / "put those counters on …": for each kind of
     * counter the referenced object has, or had as it last existed on the
     * battlefield (CR 608.2h), put that many counters of that kind on `on`.
     * Nothing is moved: counters of an object that changed zones ceased to
     * exist (CR 122.2), so this is never move-counters (122.5).
     * `those-counters` is the printed reference to counters an earlier clause
     * of the same ability named ("if it had counters on it, put those
     * counters on …"); that antecedent is not resolved here. */
    | { effect: "put-counters-of"; counters: { of: ObjectRef } | { ref: "those-counters"; scope: "contextual" }; on: ObjectRef }
    /** CR 122.5: remove counters from `from`, then put them onto `to`. If either
     * half is impossible nothing moves. `"any"` is any kind of counter; with a
     * count of 1 the kind is chosen on resolution, with "all" every kind moves. */
    | { effect: "move-counters"; counter: CounterSpec | "any"; count: Amount | "all"; from: ObjectRef; to: ObjectRef }
    /** CR 601.2d / 608.2d: the division is announced with the targets (or, for
     * an untargeted group, chosen on resolution); each recipient gets at least one. */
    | { effect: "distribute-counters"; counter: CounterSpec; count: Amount; among: ObjectRef }
    | { effect: "add-mana"; who: PlayerRef; mana: ManaProduction; restriction?: ManaSpendingRestriction; riders?: ManaSpendingRider[];
        /** CR 106.4, 500.5: for this long, the mana produced here doesn't
         * empty from that player's pool. Until end of turn it survives the
         * end of every step and phase ("as steps and phases end"); until
         * end of combat it survives only the steps of that combat phase
         * ("as steps end") and empties when the phase ends. */
        keptUntil?: { duration: "end-of-turn" | "end-of-combat" } }
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
    /** CR 701.14a: two already identified creatures ("those creatures",
     * "the chosen creatures") fight each other; each deals damage equal to
     * its power to the other. `what` is the plural reference to the pair. */
    | { effect: "fight-each-other"; what: ObjectRef }
    | { effect: "modal"; count: ModalCount; options: ModalOption[]; allowRepeat?: true;
        /** The modes are selected by the game at random, not by a player. */
        random?: true;
        keyword?: "spree" | "tiered";
        /** The same modal ability on this source remembers chosen modes;
         * changing zones creates a new source object (CR 400.7). */
        unchosen?: { period: "turn" | "source-object" };
        /** "If [condition], [you may] choose both instead": if the condition
         * holds as the modes are chosen (CR 601.2b, 700.2a–b), this count
         * replaces `count`. "You may choose both" keeps the smaller number
         * allowed (min/max); "choose both" requires the larger (exactly). */
        conditionalCount?: { condition: Condition; count: ModalCount };
        /** "Each mode must target a different player": every chosen mode has
         * a targeted player, and no player is the target of two of them. */
        distinctTargetPlayers?: true;
        /** "An opponent chooses one —": a player other than the controller
         * chooses the modes when the controller normally would; for "an
         * opponent" the controller decides which opponent (CR 700.2e). */
        chooser?: PlayerRef }
    /** CR 707.10: put count copies of EACH selected spell/activated/triggered
     * ability onto the stack under who's control, without casting/activating.
     * Retain original decisions and, for abilities, the original source. */
    | { effect: "copy-stack-object"; what: ObjectRef; who: PlayerRef; count: Amount;
        /** CR 707.10c: choose for each copy before placing it on the stack;
         * declining preserves copied targets. Does not change modes/division. */
        retarget?: { who: PlayerRef; optional?: true };
        /** CR 707.9a–d, 707.10: "except the copy isn't legendary": copiable
         * characteristics of every copy that differ from the original's. */
        copyExceptions?: CopyCharacteristicChange[] }
    /** CR 707.12, 112.2a: `who` makes `count` copies of EACH referenced card
     * in the zone that card is in; the card stays where it is. Nothing is
     * put onto the stack: a copy becomes a spell only if a following
     * instruction casts it (`copy-result`), and one that isn't cast ceases
     * to exist (704.5e). The copies are owned by `who`. */
    | { effect: "copy-card"; what: ObjectRef; who: PlayerRef; count: Amount }
    /** CR 115.7d: preserve modes/division while optionally rechoosing targets
     * of every referenced object; the printed referent is never implicit. */
    | { effect: "choose-new-targets"; what: ObjectRef; who: PlayerRef;
        timing: "resolution" | "copy-creation" }
    /** CR 115.7a–c: `who` changes targets of the referenced spell or ability.
     * Each changed target must become another legal target; a target that
     * can't be changed that way stays as it was. scope "all" is the printed
     * "the target(s)" (every target changes or none does), "one" is "a
     * target" (only one of them may change), "any" is "any targets" (any
     * number of them). `to` is the only permitted new target; `newTarget` is
     * a printed restriction on it. Not choose-new-targets (115.7d), where
     * leaving targets unchanged is always a free choice. */
    | { effect: "change-targets"; what: ObjectRef; who: PlayerRef; scope: "all" | "one" | "any";
        to?: ObjectRef | PlayerRef; newTarget?: { kind: "player" } | { kind: "object"; filter: ObjectFilter } }
    /** CR 612.1–2, 613.1c (layer 3): a text-changing continuous effect on
     * the referenced objects. `who` chooses one of the listed replacements
     * and its words while the effect is applied (608.2d); every instance of
     * the old word used in that sense is replaced. See TextReplacement. */
    | { effect: "change-text"; what: ObjectRef; who: PlayerRef; replacements: TextReplacement[] }
    | { effect: "regenerate"; what: ObjectRef }
    | { effect: "prevent-damage"; prevention: DamagePrevention }
    | { effect: "redirect-damage"; redirection: DamageRedirection }
    | { effect: "prohibit"; prohibition: GameProhibition }
    | { effect: "cast-limit"; limit: CastLimit }
    | ({ effect: "targeting-as-though" } & TargetingPretense)
    /** CR 606.3: for the duration, who may activate loyalty abilities of
     * each matching permanent up to this many times in a turn. */
    | { effect: "loyalty-activation-limit"; who: PlayerRef; sources: ObjectRef; maximum: number; duration: Duration }
    /** CR 116.2c: for as long as the continuous effects this same resolution
     * created without a stated duration last, `who` may pay `costs` as a
     * special action, any time they have priority, to end all of them at
     * once. The permission itself is always created; using it is optional. */
    | { effect: "end-effect-action"; who: PlayerRef; costs: Cost[]; ends: "open-ended-effects-of-this-resolution" }
    /** Created on resolution for its stated duration (CR 611.2a). */
    | { effect: "player-rule"; rule: PlayerRule }
    /** CR 723.1, 723.3, 723.5: `who` controls `controlled` during that
     * player's next turn: the next turn that player actually takes (a skipped
     * turn postpones it, 723.1b), the whole turn. `who` makes every choice and
     * decision `controlled` would make; only control of the player changes,
     * not of objects. A later such effect on the same player overwrites it
     * (723.1a). */
    | { effect: "control-player"; who: PlayerRef; controlled: PlayerRef; during: { turn: "next"; whose: PlayerRef } }
    /** See the static of the same name; lasts for the stated duration. */
    | { effect: "legend-rule-exemption"; what?: ObjectRef }
    /** A payment adjustment created on resolution for its stated duration
     * (CR 601.2f, 611.2a); it never changes a mana cost or mana value. */
    | ({ effect: "cost-modification"; scope: CostModificationScope } & CostModification)
    /** CR 611.2f: a continuous effect on the next matching spell the caster
     * casts during the duration (the same pending cast scope as a resolving
     * cost-modification with occurrence `next`). It doesn't begin when this
     * instruction resolves; it begins as that player next puts a matching
     * spell onto the stack (601.2a), applies to that spell only, and is used
     * up by it. Unused, it ends with the duration. */
    | { effect: "modify-spells-cast"; scope: Extract<CostModificationScope, { action: "cast" }>;
        modifiers: Extract<StaticModifier, { modifier: "abilities" | "cant" }>[] }
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
    | { effect: "player-combat-rule"; who: PlayerRef; rule: PlayerCombatRule }
  );

/** "One [category] with another": one chosen word of the category is
 * replaced by a different chosen word of the same category, only where the
 * word is used in that sense (CR 612.2), never inside a card name. `to` is a
 * printed new word instead of a chosen one; `excluding` lists words the
 * chosen new word can't be. */
export interface TextReplacement {
  category: "color-word" | "basic-land-type" | "creature-type";
  to?: string;
  excluding?: string[];
}

/** A complete instruction sentence (or several) standing where a single
 * instruction would: each sentence keeps its own steps, condition and
 * definitions. Used by self-replacement (CR 614.15) when the replaced part or
 * its replacement is more than one unconditional instruction. */
export interface InstructionSequence { sequence: Sentence[] }

/** CR 608.2c, 701.6a: later text modifies the counter event itself. A spell
 * actually countered by that instruction is put `to` instead of into its
 * owner's graveyard; a spell that isn't countered doesn't move. `only`
 * limits this to matching countered spells (Desertion); `controller` is
 * printed for a battlefield destination. Never a later zone-change effect. */
export interface CounteredSpellDestination { to: ZoneRef; only?: ObjectFilter; controller?: "you" }

export interface PileSpec { facing?: "up" | "down" }

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
  /** CR 509.4: enters blocking the named attacking creature. */
  blocking?: ObjectRef;
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
  /** "choose X" / "choose up to X" (upTo: any number from zero to X).
   * "that-many" is the symbolic anaphoric number ("choose up to that many"
   * after a repeated payment, see pay-cost `times`). */
  | { variable: "x" | "that-many"; upTo?: true }
  /** "one or more"; 0 is "choose any number" (CR 700.2b: possibly none). */
  | { atLeast: number }
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
  /** Printed "to them" alone. The pronoun does not establish whether the
   * recipient is a player ("that player … deals 2 damage to them") or a set
   * of objects (CR 109.1); binding decides, as for `{ recipient: "they" }`. */
  | { to: "them" }
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
  | { during: "next-turn"; whose: PlayerRef }
  /** Each of the stated player's first `count` turns of the game ("your first,
   * second, or third turns"; "your first seven turns"). A player's turns are
   * counted as they take them (CR 500.1, 103.8). */
  | { during: "first-turns"; whose: PlayerRef; count: number };

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
  /** CR 101.2, 609.1: no effect of a spell or ability matching `by` can make
   * `who` perform one of `actions` on an object matching `what` (an
   * instruction to do so doesn't do it to those objects). Actions the player
   * takes for any other reason are not restricted. */
  | { prohibition: "caused-action"; who: PlayerRef; actions: ("sacrifice" | "exile")[]; what: ObjectRef; by: ObjectFilter }
  /** "Creature cards in graveyards can't enter the battlefield": no matching
   * object may be moved onto the battlefield by any means (CR 101.2). */
  | { prohibition: "enter-battlefield"; what: ObjectRef }
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

/** CR 613.11: a continuous effect that changes a rule of the game for the
 * stated players, not any object's characteristics. Each quantified player
 * is affected separately; none of these is a one-shot instruction. */
export type PlayerRule = { who: PlayerRef } & (
  /** CR 402.2, 514.1: the number the player discards down to in their
   * cleanup step, normally seven. "none" removes that discard entirely;
   * "set" replaces the number; "increase"/"reduce" adjust it. */
  | { rule: "maximum-hand-size"; change:
        | { operation: "none" }
        | { operation: "set"; value: Amount }
        | { operation: "increase" | "reduce"; amount: Amount } }
  /** CR 119.7 / 119.8: also stops exchanges and redistributions that would
   * raise / lower the life total and costs involving that gain / payment. */
  | { rule: "cant-gain-life" | "cant-lose-life" }
  /** CR 121.3: the player can't draw cards (an effect that offers the draw
   * can't be chosen); CR 701.23: can't search any library; CR 702.29a:
   * can't cycle cards, i.e. can't activate cycling abilities. */
  | { rule: "cant-draw-cards" | "cant-search-libraries" | "cant-cycle-cards" }
  /** CR 101.2, 104.2b, 104.3: overrides every rule or effect by which the
   * player would win / lose, except 104.2a and conceding (104.3a). In a team
   * game it applies to the player's whole team (810.8a). */
  | { rule: "cant-win-game" | "cant-lose-game" }
  /** The state-based loss of CR 704.5a (0 or less life) alone doesn't apply
   * to the player; every other way of losing still does. */
  | { rule: "no-loss-at-zero-life" }
  /** CR 500.5, 106.4: this unspent mana of the player doesn't empty as
   * steps and phases end. A color limits it to mana of that color. */
  | { rule: "keep-unspent-mana"; mana: "any" | Color }
  /** CR 701.38d: in each vote, the player has `additional` more votes, cast
   * at the same time as their usual one. `optional`: "may vote an additional
   * time"; otherwise "gets an additional vote". */
  | { rule: "additional-votes"; additional: number; optional: boolean }
);

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
    /** "divided": the amount is divided among the targets as announced
     * (CR 601.2d); each target gets a shield of its share. */
    | { shield: "damage-budget"; amount: Amount; allocation?: "per-recipient" | "divided" }
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

/** CR 614.9: a redirection effect replaces damage that would be dealt to one
 * recipient with the same damage dealt to another: same source, same amount,
 * still combat or noncombat. Nothing is prevented (615.1). `eventScope`,
 * `kind`, `by` and `to` describe the watched damage exactly as they do on
 * DamagePrevention. A damage budget redirects each 1 damage until it is used
 * up, like a shield (615.7); a "next" event is one instance of damage of any
 * size (615.8). If the new recipient can no longer be dealt damage when the
 * damage would be redirected, the effect does nothing. */
export interface DamageRedirection {
  eventScope: DamagePrevention["eventScope"];
  shield:
    | { shield: "damage-budget"; amount: Amount }
    | { shield: "events"; occurrence: "each" | "next" };
  kind: "all" | "combat" | "noncombat";
  by?: ObjectRef | DamageSourceChoice;
  to?: (ObjectRef | PlayerRef)[];
  /** The new recipient. A participant of the watched damage is a
   * replacement-event-object: its source or its original recipient. */
  redirectTo: ObjectRef | PlayerRef;
  /** Chosen each time the damage would be dealt, not when the effect is created. */
  optional?: { who: PlayerRef };
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
  /** Printed "this ability" in a copy exception: the ability whose own
   * instruction creates the copy effect, kept by the copy (CR 707.9a). */
  | { gained: "this-ability" }
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
  /** "loses this ability": only the ability whose resolution creates this
   * effect, on the object that has it; every other ability stays. */
  | { removal: "this-ability" }
  /** A keyword without its parameter ("landwalk", "bands with other") names
   * every ability of that family: "loses all landwalk abilities" (CR 702.14a,
   * 702.22b). */
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
  attackingDefender?: PlayerRef | ObjectRef;
  blocking?: ObjectRef;
} | {
  copyOf: ObjectRef;
  modifications?: CopyModification[];
  tapped?: boolean;
  attacking?: boolean;
  attackingDefender?: PlayerRef | ObjectRef;
  blocking?: ObjectRef;
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
  /** CR 508.4: with `attacking`, the defending player the effect names
   * ("attacking that player"); absent, its controller chooses. */
  attackingDefender?: PlayerRef | ObjectRef;
  /** CR 509.4: put onto the battlefield blocking the named attacking
   * creature ("that's blocking that creature"). */
  blocking?: ObjectRef;
}

export interface BecomeSpec {
  copyOf?: ObjectRef;
  /** Only with copyOf. CR 707.9a–d: exceptions stated with the copy effect
   * are part of the copiable values it gives the object. */
  copyExceptions?: CopyCharacteristicChange[];
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
 * (CR 701.60) are status-change prohibitions. "be-sacrificed" (CR 701.21a,
 * 101.2): no player can sacrifice the permanent, whether an effect instructs
 * it or a cost asks for it; such a cost can't be paid with it. */
/** "be-copied": CR 113.6g, functions while the object is on the stack. */
/** "be-enchanted" / "be-equipped" (CR 303.4c, 301.5c, 701.3a-b, 704.5n): no
 * Aura / Equipment can become attached to the permanent; one already
 * attached is attached illegally, so the Aura is put into its owner's
 * graveyard and the Equipment becomes unattached. */
export type CantAction = "attack" | "block" | "attack-or-block" | "be-blocked" | "untap" | "be-countered" | "be-copied" | "be-regenerated"
  | "phase-in" | "phase-out" | "transform" | "be-turned-face-up" | "become-suspected" | "be-sacrificed" | "be-enchanted" | "be-equipped"
  /** CR 115.1, 601.2c, 602.2b, 603.3d: the object can't be chosen as a
   * target of a spell or ability matching `by` (one alternative of an anyOf
   * suffices); an object that becomes such while targeted is an illegal
   * target on resolution (608.2b). It is not hexproof or shroud (702.11b,
   * 702.18a) even where the wording matches their definitions, so effects
   * that refer to those keywords do not see it. */
  | { action: "be-targeted"; by: ObjectFilter };

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
  | { rule: "attack-requirement"; frequency: "each-combat" | "each-turn"; defender?: CombatDefender;
      /** CR 508.1d: the requirement exists only for a declaration in which
       * a creature matching this is declared as an attacker ("If a creature
       * you control attacks, this creature also attacks if able"). */
      whenAttacking?: ObjectRef }
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
  /** CR 508.1c, 509.1b: "can't attack/block unless [N] other [creatures]
   * also attack/block": a restriction on the whole declaration. At least
   * `minimum` creatures matching `others` (never the affected creature
   * itself) must be declared as attackers/blockers in the same declaration. */
  | { rule: "declared-with"; action: "attack" | "block"; others: ObjectFilter; minimum: Amount }
  /** CR 506.5: alone means the only creature declared as an attacker or as
   * a blocker in that declaration. exclude forbids being declared alone;
   * only forbids being declared together with any other creature. */
  | { rule: "alone-restriction"; action: "attack" | "block"; mode: "exclude" | "only" }
  /** CR 609.4: the affected creature can be declared as an attacker (508.1)
   * or blocker (509.1), or can be blocked, as though the pretense were true:
   * without defender (702.3b), with haste (302.6, 702.10b), with or without
   * an evasion ability (509.1b), untapped (509.1a). It lifts no other
   * restriction and creates no requirement. defenders limits the attacks,
   * and attackers the blocked creatures, that the pretense applies to. */
  | { rule: "as-though"; action: "attack" | "block" | "be-blocked"; pretense: CombatPretense;
      defenders?: CombatDefender[]; attackers?: ObjectRef }
  /** CR 510.1a: the affected creature assigns combat damage equal to this
   * characteristic of its own rather than its power. Its power is unchanged. */
  | { rule: "damage-assignment-value"; value: "toughness" | "mana-value" }
  /** CR 510.1b–c, 609.4: the affected creature's combat damage is assigned
   * as though it weren't blocked. With chooser ("[player] may have it
   * assign"), that player decides whether it is; otherwise it always is. */
  | { rule: "assign-as-unblocked"; chooser?: PlayerRef }
  /** CR 510.1, 611.2a: the affected creature assigns no combat damage at
   * all for the effect's period; it is neither redirected nor prevented. */
  | { rule: "assigns-no-combat-damage" }
  /** CR 508.1h, 509.1d: a cost to attack or block. An affected creature can
   * be declared as an attacker (of `defenders`, when stated) or as a blocker
   * (of `attackers`, when stated) only if `who` pays while making that
   * declaration. Paying is never required (508.1d, 509.1c); this is not an
   * "unless" choice on resolution (118.12a). With scope
   * "each-affected-creature", `costs` (times `multiplier`) is owed once for
   * every affected creature declared, and `who` and `multiplier` are read
   * for that creature. With scope "declaration", `costs` times `multiplier`
   * is the printed total for one player's declaration; that multiplier
   * counts creatures as they are being declared. */
  | { rule: "declaration-cost"; action: "attack" | "block" | "attack-or-block"; who: PlayerRef; costs: Cost[];
      scope: "each-affected-creature" | "declaration"; multiplier?: Amount;
      defenders?: CombatDefender[]; attackers?: ObjectRef;
      bindings?: { variable: "x"; value: Amount }[] }
  | { rule: "all-of"; rules: CombatRule[] }
);

/** Restrictions and requirements on a player's attack or block declaration
 * as a whole (CR 508.1c–d, 509.1b–c), not on particular creatures. */
export type PlayerCombatRule =
  /** The player's creatures can't attack any of these defenders. */
  | { rule: "attack-restriction"; defenders: CombatDefender[] }
  /** CR 508.1d: the player must declare at least `minimum` attackers each
   * combat if able; with `defenders`, they must attack one of these. */
  | { rule: "attack-requirement"; minimum: number; defenders?: CombatDefender[] }
  /** CR 509.1b: the player can't declare blockers beyond this many creatures. */
  | { rule: "block-limit"; maximum: Amount }
  /** CR 508.1c, 802.3a: the player can't declare more than this many
   * attackers; with `defenders`, only creatures attacking one of them count. */
  | { rule: "attack-limit"; maximum: Amount; defenders?: CombatDefender[] };

/** CR 609.4, 702.11b–e, 702.18a: the listed permanents and players can be
 * chosen as targets of spells and abilities as though they didn't have the
 * stated ability (for hexproof, any "hexproof from" ability too). `by`
 * limits this to spells and abilities that player controls (absent: any).
 * Nothing else about the permanents or players changes. */
export interface TargetingPretense {
  what: (ObjectRef | PlayerRef)[];
  lacking: "hexproof" | "shroud";
  by?: PlayerRef;
}

/** CR 609.4: a counterfactual that holds only for the combat action of the
 * rule carrying it. `of` is the affected creature itself or, for a block,
 * each attacking creature it would block. A landwalk keyword without a
 * filter stands for every landwalk ability (702.14a). */
export type CombatPretense =
  | { pretend: "has-keyword" | "lacks-keyword"; of: "affected-creature" | "blocked-creature"; keyword: KeywordInstance }
  | { pretend: "untapped"; of: "affected-creature" };

export type ManaProduction =
  | { mana: "fixed"; symbols: string[] }                    // Add {G}{G}
  | { mana: "choice"; options: ManaProduction[] }           // Add {R} or one mana of the chosen color
  | { mana: "chosen-color"; amount: Amount; choice: ChosenValueRef<"color"> }
  | { mana: "any-color"; amount: Amount }                   // Add two mana of any color
  /** "Add two mana of different colors": each unit is a different color (CR 105.1, 106.1a). */
  | { mana: "different-colors"; amount: Amount }
  /** "one mana of any color among legendary permanents you control": a color
   * that one of those objects is; if none is colored, no mana (CR 106.5). */
  | { mana: "color-among"; amount: Amount; among: ObjectRef }
  /** "For each color among permanents you control, add one mana of that
   * color": one mana of each color that one of those objects is (CR 106.5). */
  | { mana: "each-color-among"; among: ObjectRef }
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
  | { use: "activate"; source?: ObjectFilter;
      /** Only abilities this keyword defines: equip (CR 702.6a), power-up (702.193a). */
      abilityKeyword?: "equip" | "power-up" }
  /** Any payment of a cost the named keyword defines: cumulative upkeep
   * (CR 702.24a), disturb (702.146a) or morph (702.37e). */
  | { use: "pay-keyword-cost"; keyword: "cumulative upkeep" | "disturb" | "morph" }
  /** Any cost that includes this symbol: {X} (CR 107.3) or {C} (107.4c). */
  | { use: "pay-cost-containing"; symbol: "X" | "C" }
  /** The generic component of any cost (CR 107.4b). */
  | { use: "pay-generic-costs" }
  /** CR 116.2b, 708: the special action of turning a matching face-down
   * permanent face up, whichever cost that takes. */
  | { use: "turn-face-up"; what: ObjectFilter }
  /** CR 116.2h, 702.143a: the special action of foretelling a card. */
  | { use: "foretell" }
  /** CR 116.2m, 709.5e: paying an unlock cost of a locked half. */
  | { use: "unlock-door" }
  /** Activated abilities of one referenced object ("this artifact's
   * abilities"), not of every object matching a description. */
  | { use: "activate-abilities-of"; source: ObjectRef };

/** CR 106.6: besides restrictions, produced mana can carry an additional
 * effect on the spell or ability it is spent on, or a delayed triggered
 * ability that triggers when it is spent (603.7a). Each unit of the produced
 * mana carries its own copy (106.6a); "any-of-that" is the printed wording
 * for several units. In `effects`, "it" / "that spell" / "that creature"
 * denote the spell or ability the mana was spent on and, for a permanent
 * spell, the permanent it becomes; they stay symbolic. */
export type ManaSpendingRider =
  | { rider: "effect"; mana: "that" | "any-of-that"; spentOn: ManaSpendingUse[]; effects: Sentence[] }
  | { rider: "delayed-trigger"; spentOn: ManaSpendingUse[]; effects: Sentence[] }
  /** "If that mana is spent to cast a creature spell, that creature enters
   * with an additional +1/+1 counter on it": the spell's permanent enters
   * with counters (CR 106.6, 614.12). `effect.what` refers to that spell. */
  | { rider: "entry"; mana: "that"; spentOn: ManaSpendingUse[]; effect: Extract<StaticEffect, { static: "enters-with-counters" }> };

/** CR 123.1: the four kinds of stickers. */
export type StickerKind = "any" | "name" | "ability" | "power-toughness" | "art";

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
  /** "except its name is [this card's name]": the copy's name is the printed
   * name of the object this ability is on, instead of the copied name
   * (CR 707.9b). A literal name, not a reference to that object (201.5). */
  | { change: "name"; name: "source-printed-name" }
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
  /** CR 721.2a-b: one printed station symbol {N+} and its striation are a
   * static ability. While the source has `threshold` or more charge counters
   * it has these abilities. `creature` marks the striation holding the
   * printed power/toughness box: the source is then also a creature, in
   * addition to its other types, with that box's base power and toughness.
   * Oracle text does not carry the box's numbers; they are the card's
   * printed power/toughness in source metadata (721.2c), and the striation
   * is the one the Oracle reminder names ("It's an artifact creature at
   * N+."). The symbol and its contents are copiable; counters are not. The
   * station keyword itself stays an ordinary keyword line (721.4). */
  | { static: "station-band"; threshold: number; abilities: Ability[]; creature?: { basePT: "printed-box" } }
  | { static: "replacement"; replacement: ReplacementInstruction }
  | ({ static: "cost-modification"; scope: CostModificationScope } & CostModification)
  /** CR 118.8, 601.2b: "As an additional cost to cast [spells], you may pay
   * [cost]": every matching spell offers this additional cost to its caster
   * (Defilers). A condition `additional-cost-paid` tests whether it was paid. */
  | { static: "spell-additional-cost"; scope: Extract<CostModificationScope, { action: "cast" }>; costs: Cost[]; optional?: true }
  | { static: "as-enters"; what: ObjectRef; effects: Sentence[] }
  /** CR 708.11: an "As [this permanent] is turned face up" ability is applied
   * while it is being turned face up, not afterward and not as a trigger.
   * `alsoEnters` is "As this creature enters or is turned face up". */
  | { static: "as-turned-face-up"; what: ObjectRef; effects: Sentence[]; alsoEnters?: true }
  /** "As this permanent transforms into ~, …": applied while the permanent
   * transforms into the face bearing this text, like a replacement choice
   * (CR 701.27a, 701.27e, 614.1c), not as a trigger. */
  | { static: "as-transforms"; what: ObjectRef; into: { face: "source-face" }; effects: Sentence[] }
  /** Optional payment/reveal before entry, with a declined-payment entry
   * modification. This is one replacement choice, not a tap instruction.
   * CR 614.1c, 614.12a–b. */
  | { static: "entry-choice"; what: ObjectRef; who: PlayerRef; costs: Cost[]; declined: { tapped: true } }
  | { static: "prevent-damage"; prevention: DamagePrevention }
  | { static: "redirect-damage"; redirection: DamageRedirection }
  | { static: "prohibit"; prohibition: GameProhibition }
  | { static: "cast-limit"; limit: CastLimit }
  | { static: "player-rule"; rule: PlayerRule }
  /** CR 704.5j: the "legend rule" state-based action ignores the matching
   * permanents (they neither force a choice nor are put into a graveyard by
   * it); with no `what` it doesn't apply to anything. No other rule about
   * legendary permanents changes. */
  | { static: "legend-rule-exemption"; what?: ObjectRef }
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
  /** "Once during each of your turns, you may play a land or cast a spell
   * from among …": each use of this permission is exactly one of the listed
   * actions, and `useLimit` counts the uses of all of them together. The
   * listed permissions carry no limit of their own. */
  | { static: "play-permission-choice"; permissions: CardPlayInstruction[]; useLimit: NonNullable<CardPlayInstruction["useLimit"]> }
  | { static: "look-permission"; permission: LookPermission }
  /** CR 113.6e, 601.3: restricts when this particular object can begin to be
   * cast. It functions in every zone the card could be cast from and on the
   * stack; it is never an activation restriction or a condition checked on
   * resolution. `windows` are alternatives (any one suffices); `condition`
   * must also be true at that moment. */
  | { static: "cast-restriction"; what: Extract<ObjectRef, { ref: "self" | "this" }>; windows?: CastWindow[]; condition?: Condition }
  /** "While an opponent is choosing targets as part of casting a spell they
   * control or activating an ability they control, that player must choose
   * at least one Flagbearer on the battlefield if able": whenever each such
   * player chooses targets for a spell they cast (CR 601.2c) or an ability
   * they activate (602.2b), at least `minimum` of those targets match
   * `mustChoose` when the targeting rules allow it. Triggered abilities
   * (603.3d) are not covered. */
  | { static: "target-choice-requirement"; who: PlayerRef; when: ("casting-spell" | "activating-ability")[];
      mustChoose: ObjectFilter; minimum: Amount; ifAble: true }
  | { static: "additional-land-plays"; allowance: LandPlayAllowance }
  /** CR 609.4, 302.6, 702.10c: who may activate abilities of the matching
   * permanents as though those permanents had haste. Nothing else about
   * them changes; in particular they don't gain haste for attacking. */
  | { static: "activation-as-though-haste"; who: PlayerRef; sources: ObjectRef }
  /** CR 606.3: who may activate loyalty abilities of each matching permanent
   * up to this many times in a turn, rather than only once. */
  | { static: "loyalty-activation-limit"; who: PlayerRef; sources: ObjectRef; maximum: number }
  /** CR 304.5, 602.5d, 606.3: who may activate the matching abilities any
   * time they could cast an instant (they have priority), instead of only
   * at sorcery speed; `anyPlayersTurn`: also during other players' turns.
   * Every other restriction (a loyalty ability's once each turn) remains. */
  | { static: "activation-timing"; who: PlayerRef; abilities: ObjectFilter; timing: "instant"; anyPlayersTurn?: true }
  | ({ static: "targeting-as-though" } & TargetingPretense)
  /** CR 609.4b: who may pay costs with the stated mana as though it were
   * mana of that kind. This changes neither the cost nor the mana actually
   * spent. `mana` limits the mana treated this way (absent: any mana);
   * `uses` lists the payments it applies to, as alternatives (absent: every
   * cost that player pays). */
  | { static: "mana-spending"; who: PlayerRef; mana?: { color: Color };
      as: "any-color" | "any-type" | { color: Color }; uses?: ManaSpendingUse[] }
  /** CR 401.5–6, 701.20a: cards in this hidden zone position stay revealed
   * to all players while the ability functions; it is not a one-shot reveal
   * instruction. A newly exposed library top is revealed only after the
   * current casting, activation, or special action has finished (401.5).
   * Each affected player's own zone is revealed. */
  | { static: "play-revealed"; who: PlayerRef; what: "library-top" | "hand" }
  /** CR 121.1, 701.20a: as `who` draws the first card they draw in each turn
   * (each of their own turns, with `turns: "your"`), they reveal it;
   * `optional`: they may. Neither the draw nor the card's zone changes, and
   * no other draw is revealed. */
  | { static: "reveal-first-draw"; who: PlayerRef; turns?: "your"; optional?: true }
  /** CR 903.3a / 113.6n: eligibility during deck construction, not an
   * in-game designation, characteristic, or controller-based permission. */
  | { static: "commander-eligibility"; what: Extract<ObjectRef, { ref: "self" | "this" }>; during: "deck-construction" }
  /** CR 903.4b, 607.2p: if `what` is its owner's commander, `who` makes this
   * choice before the game begins and reveals it as the commander is put
   * into the command zone. The choice applies during deck construction and
   * for the whole game, across zone changes, and affects color identity.
   * `linked` is the characteristic-defining ability printed in the same
   * paragraph; its chosen value refers only to this choice. */
  | { static: "commander-pregame-choice"; what: Extract<ObjectRef, { ref: "self" }>; who: PlayerRef; choice: ValueChoice; linked: StaticEffect }
  /** CR 103.6a: a pregame action with this card from its owner's opening
   * hand, taken after mulligans in turn order. `who` may put the card onto
   * the battlefield so the game begins with it there; this is a permission,
   * never a resolving instruction or a replacement of a later entry. */
  | { static: "opening-hand-action"; what: Extract<ObjectRef, { ref: "self" | "this" }>; who: PlayerRef; action: "begin-on-battlefield" }
  /** CR 103.6b: `who` may reveal this card from their opening hand as a
   * pregame action; it stays revealed until the first turn begins and each
   * card may be revealed this way once. `effects` is the "If you do, ..."
   * instruction, performed only when the card was revealed. */
  | { static: "opening-hand-action"; what: Extract<ObjectRef, { ref: "self" | "this" }>; who: PlayerRef; action: "reveal"; effects: Sentence[] }
  /** CR 103.3: just before `who` shuffles their deck to start the game, they
   * may reveal this card from that deck and, as the same optional action,
   * perform `effects` (each instruction's own choices are made then). The
   * card stays in the deck; this is not an opening-hand action (103.6). */
  | { static: "pregame-deck-reveal"; what: Extract<ObjectRef, { ref: "self" | "this" }>; who: PlayerRef; effects: Effect[] }
  /** CR 100.2a / 113.6n: replaces the four-copy limit for cards with this
   * English name during deck construction (deck and sideboard combined,
   * 100.4a). Absent maximum means any number. The name is the printed
   * literal, not a reference to whatever this object is currently named. */
  | { static: "deck-copy-limit"; name: { name: "literal"; value: string }; maximum?: number; during: "deck-construction" }
  /** CR 502.3: the chooser may omit this object from the simultaneous
   * turn-based untap action; this never prohibits untapping by an effect. */
  | { static: "optional-untap"; who: PlayerRef; what: ObjectRef; step: { occurrence: "each"; owner: PlayerRef } }
  /** CR 502.3: while this ability functions, the matching permanents also
   * untap during the untap step of each stated player, as part of that
   * step's turn-based untap action. It is not a trigger, uses no stack, and
   * is mandatory; effects that keep a permanent from untapping during an
   * untap step still apply to it. */
  | { static: "untap-during-step"; what: ObjectRef; step: { occurrence: "each"; owner: PlayerRef } }
  /** CR 502.3: during each of their own untap steps, each affected player
   * untaps no more than `maximum` of the matching permanents they control,
   * choosing which. Untapping by spells and abilities is not limited. */
  | { static: "untap-step-limit"; who: PlayerRef; what: ObjectFilter; maximum: Amount }
  /** The one spell that is the stated ordinal among the caster's matching
   * casts of a turn has these abilities from the moment it begins to be cast
   * (CR 601.2), so its cast triggers and cost keywords apply. Every other
   * matching spell is unaffected. `scope.occurrence` is always present;
   * unordered grants to spells stay ordinary `modify` effects. */
  | { static: "cast-spell-abilities"; scope: Extract<CostModificationScope, { action: "cast" }>; abilities: GainedAbility[] }
  /** CR 118.9: an optional casting payment, never a resolution instruction.
   * useLimit: this alternative cost can be chosen for only that many of the
   * matching spells in each turn (`during: "your"`: only in who's own turns). */
  | { static: "alternative-cost"; who: PlayerRef; spells: ObjectRef; costs: Cost[]; condition?: Condition;
      useLimit?: { maximum: number; period: "turn"; during: "your" | "any" };
      /** Limits the mana that may pay this alternative cost ("... to cast it this way"). */
      paymentRestriction?: PaymentRestriction }
  /** CR 609.4, 120.3b/d: damage (only damage dealt to `to`, when stated) has
   * the results it would have if its source had this keyword. No source
   * gains the keyword; nothing else about the damage changes. */
  | { static: "damage-as-though"; keyword: "wither" | "infect"; to?: PlayerRef }
  /** CR 602.2b, 601.2b/f: an alternative to the activation cost of each
   * activation in `scope` (the analog of a spell's alternative cost, 118.9),
   * chosen as the ability is activated. Additional costs, increases and
   * reductions still apply to it. Nothing else about the ability changes. */
  | { static: "alternative-activation-cost"; who: PlayerRef; scope: Extract<CostModificationScope, { action: "activate" }>; costs: Cost[] }
  /** A limit on the mana that may pay this spell's own cost; it functions
   * wherever the spell can be cast from (CR 113.6e-style casting rule). */
  | { static: "payment-restriction"; what: Extract<ObjectRef, { ref: "this" }>; restriction: PaymentRestriction }
  /** CR 107.3a/f: limits on the X this spell's controller chooses, printed
   * as their own paragraph ("X can't be 0."). Like a payment restriction it
   * is a casting rule of the spell, not a resolving instruction. */
  | { static: "variable-constraints"; what: Extract<ObjectRef, { ref: "this" }>; constraints: VariableConstraint[] }
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
  /** CR 722.3a/c: the permanent has the prepared designation as it enters
   * (if it has a prepare spell); this modifies the entry event and is never
   * a later "becomes prepared" instruction. */
  | { static: "enters-prepared"; what: ObjectRef }
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
  /** CR 614.1c, 614.12a: the permanent enters with the counters of `count`
   * different `options`, chosen by `who` before it enters ("your choice of a
   * flying counter or a hexproof counter"; "two different counters from
   * among …"). Unchosen options put nothing. `condition` is tested as it
   * would enter, as on enters-with-counters. */
  | { static: "enters-with-counter-choice"; what: ObjectRef; who: PlayerRef; count: number;
      options: { counter: CounterSpec; count: Amount }[]; condition?: Condition; conditionKind?: "if" }
  /** CR 301.5, 303.4c, 701.3a-b, 704.5n: besides its own enchant or equip
   * rules, `what` can legally be attached only to an object matching `to`.
   * An effect that would attach it to anything else leaves it where it is;
   * attached to a nonmatching object, it becomes unattached (Equipment,
   * Fortification) or is put into its owner's graveyard (Aura) as a
   * state-based action. */
  | { static: "attachment-limit"; what: Extract<ObjectRef, { ref: "self" | "this" }>; to: ObjectFilter }
  | { static: "cant"; what: ObjectRef; action: CantAction; condition?: Condition; conditionKind?: "if" | "unless" }
  | { static: "combat-rule"; what: ObjectRef; rule: CombatRule }
  | { static: "player-combat-rule"; who: PlayerRef; rule: PlayerCombatRule }
  /** CR 702.122a–b, 702.171a/c, 702.184a/c: when the affected creature is
   * tapped to pay a crew or saddle cost, or to station a permanent, it
   * counts with this value in place of its power: its power plus a number
   * ("as though its power were N greater", 609.4) or its toughness. Its
   * power is unchanged for every other purpose. */
  | { static: "crew-contribution"; what: ObjectRef; actions: ("crew" | "saddle" | "station")[];
      value: { value: "power-plus"; amount: number } | { value: "toughness" } }
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
  /** CR 113.6i: counters of this kind can't be put on the object by any
   * cost or effect, also as it enters; counters already on it remain. */
  | { modifier: "cant-have-counters"; counter: CounterSpec | "any" }
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
  | { action: "cast"; spells: ObjectRef; caster?: PlayerRef; from?: ZoneRef | { anyOf: ZoneRef[] };
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
  | { action: "pay-cost"; cost: "flashback" | "buyback" | "dash" | "blitz" | "morph" | "unlock"; payer?: PlayerRef };

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
  /** "Can't reduce the amount of mana this spell costs by more than {5}":
   * the reduction itself is limited to this many mana (CR 601.2f). */
  maximumReduction?: Amount;
  coloredOnly?: true;
  /** "This effect reduces only the amount of [color] mana you pay": a colored
   * reduction that is never converted to generic mana (CR 118.7b) when the cost
   * lacks that color. */
  onlyColor?: Color;
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
  /** The card can be cast this way only by using this keyword ability of
   * its own, for that ability's alternative cost and with its consequences
   * (CR 702.152a blitz, 702.140a mutate, 702.103a bestow, 702.185a warp). */
  usingKeyword?: "blitz" | "mutate" | "bestow" | "warp";
  /** "Cast it … as an Adventure": the adventurer card can be cast this way
   * only as an Adventure, with its alternative characteristics (CR 715.3,
   * 715.3a–b). */
  asAdventure?: true;
  /** "The next [spell] you cast this turn can be cast …": the permission
   * concerns only the first matching spell the player casts (or card the
   * player plays) after the effect is created. That spell uses the effect
   * up whether or not it needed the permission. */
  occurrence?: { next: true };
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
  | { rider: "cast-off-sorcery-timing"; effects: Sentence[] }
  /** The spell cast this way can't be countered (CR 101.2, 701.6). */
  | { rider: "cant-be-countered" }
  /** "A spell cast this way costs {2} more to cast" (CR 601.2f): adjusts the
   * total cost of a spell cast using this permission only. `caster` restricts
   * it to spells that player casts ("cast by an opponent this way"). */
  | { rider: "cost-change"; change: CostModification["change"]; caster?: PlayerRef }
  /** "If you do, it gains …": the card used this way gains the abilities —
   * the spell, and then the permanent it becomes (CR 400.7a), or the land
   * played — for `duration` when one is printed. `appliesTo` restricts it to
   * matching spells cast this way. */
  | { rider: "gain-abilities"; appliesTo?: ObjectFilter; abilities: GainedAbility[]; duration?: Duration }
  /** "When you cast a spell / play a card this way, …": a triggered ability
   * linked to the permission (CR 603.1, 607.2). It triggers each time the
   * permission is used for the printed action: `cast` only when a spell is
   * cast with it, `play` also when a land is played with it (701.18).
   * `appliesTo` restricts it to matching spells. */
  | { rider: "use-trigger"; event: "cast" | "play"; appliesTo?: ObjectFilter; effects: Sentence[] };

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
  | { event: "move-zone"; what: ObjectRef; from?: ZoneRef | "anywhere"; to?: ZoneRef;
      /** "… as it resolves" (CR 608.2n): only the instant or sorcery spell's
       * own move to the graveyard as the final part of its resolution, not a
       * spell countered or otherwise leaving the stack. */
      resolving?: true }
  /** CR 701.8a, 704.5g–h: the permanent would be destroyed, by an effect or
   * by a state-based action; not sacrificed. In a static replacement,
   * "regenerate it" is the alternate effect of CR 701.19b. */
  | { event: "destroy"; what: ObjectRef }
  /** CR 701.9a: the player would discard the card because of an effect of
   * `cause` (a spell or ability matching the reference), not because of a
   * game rule such as the cleanup discard (514.1). Moving the discarded card
   * elsewhere instead of into the graveyard still discards it (614.6). */
  | { event: "discard"; who: PlayerRef; what: ObjectRef; cause: ObjectRef }
  | { event: "damage"; source?: ObjectRef; to?: (ObjectRef | PlayerRef)[];
      kind: "all" | "combat" | "noncombat"; quantity?: Comparison }
  | { event: "gain-life" | "lose-life"; who: PlayerRef }
  /** CR 120.3a: damage dealt to the player would bring their life total to
   * less than `below`. The damage is still dealt; only the resulting life
   * total is replaced. */
  | { event: "damage-life-reduction"; who: PlayerRef; below: Amount }
  /** CR 104.3: the player would lose the game for any reason, whether by a
   * state-based action (704.5a–c) or by an effect. */
  | { event: "lose-game"; who: PlayerRef }
  | { event: "draw"; who: PlayerRef; quantity?: Comparison; except?: "first-in-each-draw-step" }
  /** CR 701.17a (mill), 701.22a (scry): the player would perform this
   * keyword action on a number of cards; `quantity` restricts that number
   * ("a number of cards" restricts nothing). */
  | { event: "mill" | "scry"; who: PlayerRef; quantity?: Comparison }
  /** CR 701.34a: the player would proliferate. */
  | { event: "proliferate"; who: PlayerRef }
  /** CR 706.1, 706.7: the player would roll dice; `die` restricts the
   * event to that kind of die (absent: any dice, as printed). */
  | { event: "roll-dice"; who: PlayerRef; quantity: Comparison; die?: DieSpec }
  /** CR 705.1: each coin the player would flip is a separate event. */
  | { event: "flip-coin"; who: PlayerRef }
  /** CR 500.11, 614.10: the player's turn or step would begin. `extra`
   * restricts the event to extra turns (CR 500.7). */
  | { event: "begin-time"; who: PlayerRef; what: SkippableTime; extra?: true }
  /** CR 106.12b: a mana ability of the permanent with {T} in its cost is
   * resolving and producing mana. `by` is the printed player who tapped it;
   * `quantity` restricts the amount of mana it would produce. */
  | { event: "tap-for-mana"; what: ObjectRef; by?: PlayerRef; quantity?: Comparison }
  /** CR 106.4, 500.5: the player's mana pool would empty of its unspent
   * mana as a step or phase ends. */
  | { event: "lose-unspent-mana"; who: PlayerRef }
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
    /** The watched damage reduces the player's life total to this value. */
    | { replacement: "reduce-life-to"; amount: Amount }
    /** CR 705, 706.6: the player rolls (flips) the watched number of dice
     * (coins) of the same kind changed by `change`, then ignores `ignore`
     * of them; an ignored roll or flip never happened. The lowest roll is
     * chosen by the roller among ties; otherwise `who` chooses. */
    | { replacement: "roll-and-ignore"; change: { operation: "add" | "set"; amount: Amount };
        ignore: { ignore: "lowest"; count: 1 } | { ignore: "chosen"; who: PlayerRef; count: 1 } }
    /** CR 106.12b: the tapped permanent produces exactly this mana instead
     * of the type and amount it would produce. */
    | { replacement: "mana"; mana: ManaProduction }
    /** CR 106.12b: each unit of mana it would produce becomes mana of this
     * type; the amount is unchanged. `only` limits this to units of that
     * color. A color "of your choice" is chosen as the replacement applies. */
    | { replacement: "mana-type"; type: { symbol: string } | { choice: "color"; who: PlayerRef }; only?: Color }
    /** CR 106.4: the unspent mana isn't lost; it stays in the player's mana
     * pool as mana of this type. */
    | { replacement: "keep-mana"; as: { symbol: string } }
    /** CR 614.10: "skip [it]" means "instead of [it], do nothing". `after`
     * follows an optional skip ("If you do, …") and happens only when the
     * skip was chosen, as the first thing in the next occurrence (614.10b). */
    | { replacement: "skip"; after?: Sentence[] }
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
  | { condition: "attacked"; who: PlayerRef; during: "this-turn" | "this-combat"; with?: ObjectFilter;
      /** "attacked with three or more creatures": how many different creatures
       * matching `with` were declared as attackers in the window (CR 508.1a). */
      count?: Comparison;
      /** "attacked with creatures with total power 6 or greater": the
       * creatures `who` attacked with in the window (matching `with`), taken
       * together, satisfy these; signed values add (CR 107.1b). */
      groupConstraints?: GroupConstraint[] }
  /** Passive counterpart: a creature was declared as an attacker against
   * this player during the current step (CR 508.1b). The printed text does
   * not say whether an attack on the player's planeswalker or battle
   * qualifies; this node asserts only the printed predicate. */
  | { condition: "was-attacked"; who: PlayerRef; during: "this-step" }
  /** Historical declared combat actions, distinct from current attacking or
   * blocking status. Shared-subject alternatives use explicit any-of nodes. */
  /** "was-blocked": it became a blocked creature (CR 509.1h), whether or
   * not it is still blocked or its blockers remain in combat. */
  | { condition: "combat-action-history"; what: ObjectRef; action: "attacked" | "blocked" | "was-blocked";
      /** `{ lastTurnOf }` is that player's own most recent previous turn
       * ("during your last turn"), not simply the turn before this one
       * (CR 800.4i uses "their last turn" this way). */
      during: "this-turn" | "this-combat" | { lastTurnOf: PlayerRef } }
  /** CR 400.7d: an entering permanent can ask about its originating spell.
   * when tests the casting event's phase, never the current resolving phase. */
  | { condition: "was-cast"; what: ObjectRef; by?: PlayerRef; from?: ZoneRef;
      /** Cast from any zone except this one ("from anywhere other than your hand"). */
      notFrom?: ZoneRef; when?: { phase: PhaseName; whose: PlayerRef } }
  /** Actual damage events, not life lost or damage still marked. Singular
   * source/recipient descriptions instantiate one matching object/player;
   * total sums matching damage over the period for those same referents. */
  | { condition: "damage-history"; source?: ObjectRef; to?: ObjectRef | PlayerRef;
      damage?: "combat" | "noncombat"; quantity: "positive-event" | "total";
      comparison: Comparison;
      /** "was dealt excess damage": only damage events that dealt the
       * recipient excess damage (CR 120.10) qualify. */
      excess?: true;
      /** object-lifetime ("hasn't dealt damage yet"): since the source became
       * this object; a zone change makes a new object with no history (400.7). */
      during: "this-turn" | "this-combat" | "last-turn" | "object-lifetime" }
  /** Mana actually spent, never the spell's cost or mana value (CR 609.4b).
   * Snow is a source quality rather than a mana color/type (CR 107.4h).
   * source is evaluated when that mana was produced (CR 106.3). */
  | { condition: "mana-spent-to-cast"; what: ObjectRef;
      mana: "any" | "colored" | Color | "colorless" | "snow"
        /** "mana of the same color" (adamant, CR 207.2c): that many mana of one color, whichever color it is. */
        | "same-color";
      comparison: Comparison; source?: ObjectFilter }
  | { condition: "has-keyword"; what: ObjectRef; keyword: string }
  /** CR 702.95b: `what` is currently paired by a soulbond ability; `with`
   * describes the creature it is paired with. A creature is paired with at
   * most one other (702.95d) and unpairing follows 702.95e. */
  | { condition: "paired"; what: ObjectRef; with: ObjectFilter }
  /** Each qualifying object movement counts separately. Filters and control
   * are evaluated at that movement, not using the object's current state.
   * A dies history is battlefield -> graveyard (CR 700.4). */
  | { condition: "zone-change-history";
      event: "dies" | "enters" | "leaves-battlefield" | "leaves-zone" | "put-into-zone";
      subject: { match: "filter"; filter: ObjectFilter; comparison: Comparison } | { match: "object"; what: ObjectRef };
      from?: ZoneRef | "anywhere"; to?: ZoneRef; controllerAtMove?: PlayerRef; during: "this-turn" | "this-combat" | "last-turn" }
  /** CR 603.6a, 400.7: the zone this permanent moved to the battlefield from
   * in the entry event that made it this object. It is never where the object
   * is now, and a resolved permanent spell entered from the stack whatever
   * zone it was cast from (that is `was-cast`). */
  | { condition: "entered-from"; what: ObjectRef; from: ZoneRef }
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
   * 702.35b, spectacle 702.137a, emerge 702.119a, freerunning 702.173a,
   * mayhem 702.187b, sneak 702.190a), or cast using teamwork (702.194b: the
   * declared intention to pay that additional cost). `what` is the printed spell or the
   * permanent it became (400.7d); when the text names no object, the query
   * belongs to the object bearing this linked ability (607.2). */
  | { condition: "keyword-choice"; keyword: LinkedChoiceKeyword; what?: ObjectRef }
  /** This spell was cast by paying the alternative cost its own text offers
   * with exactly these components (CR 118.9, 607.2). */
  | { condition: "alternative-cost-paid"; cost: Cost[] }
  /** The spell's optional additional cost with exactly these components was
   * paid as it was cast ("if you paid life this way", CR 601.2b). */
  | { condition: "additional-cost-paid"; cost: Cost[] }
  /** "This spell's additional cost was paid": the optional additional cost
   * printed on that same spell (its `additional-cost` line) was paid as it
   * was cast (CR 118.8b, 601.2b, 607.2). */
  | { condition: "additional-cost-paid"; what: ObjectRef }
  /** Resolutions of this same ability of this source during the period,
   * counting the current one; a copy counts as the same ability (CR
   * 707.10b). True when that count is any listed ordinal. */
  | { condition: "ability-resolution-ordinal"; ordinals: number[]; during: "this-turn" }
  /** CR 603.4: the event that triggered this ability is the first event
   * matching `event` during the period; any earlier matching event, whatever
   * caused it, makes this false. */
  | { condition: "first-event"; event: Trigger; during: "this-turn" }
  /** Activations of this same ability of this source during the period,
   * including the one resolving (CR 602.2). */
  | { condition: "ability-activation-count"; comparison: Comparison; during: "this-turn" }
  /** Gross life gained/lost over the period, not a net life-total change. */
  | { condition: "life-change"; who: PlayerRef; change: "gained" | "lost"; comparison: Comparison; during: "this-turn" | "last-turn" }
  /** Completed actions by one matching player during the period. The
   * comparison counts objects (sacrifice, discard, draw, create-token,
   * cycle) or occurrences (the rest). Filters describe each object as the
   * action happened, not its current state (CR 608.2i). */
  | { condition: "player-action-history"; who: PlayerRef; action: PlayerHistoryAction; comparison: Comparison;
      during: "this-turn" | "last-turn" }
  /** CR 601.2i, 400.7d: `what` (a spell, or the permanent it became) is the
   * `ordinal`-th spell matching `of` that `who` cast during the period,
   * counting spells as they were cast; copies are not cast (707.10). */
  | { condition: "cast-ordinal"; what: ObjectRef; who: PlayerRef; of: ObjectFilter; ordinal: number; during: "this-turn" }
  | { condition: "cast-count"; who: PlayerRef | "all-players-combined"; what: ObjectFilter; comparison: Comparison; during: "this-turn" | "last-turn"; from?: ZoneRef }
  /** time "previous" is the referenced object's last existence before the
   * instruction just applied to it moved or destroyed it (CR 608.2h). */
  | { condition: "control"; who: PlayerRef; what: ObjectRef; time?: "previous";
      /** "if you controlled a Faerie as you cast this spell": control at that moment. */
      asOf?: AnnouncementTime }
  | { condition: "control-count"; who: PlayerRef; of: ObjectFilter; comparison: Comparison }
  /** The same quantity measured for two player operands. Each operand keeps
   * its printed quantifier: an opponent / a player is existential, each
   * opponent / each other player universal. Every pair is compared
   * separately; quantities of several players are never summed. */
  | { condition: "player-comparison"; who: PlayerRef; quantity: PlayerQuantity; op: "gt" | "lt";
      than: PlayerRef | { player: "each-other-player" } }
  /** CR 102.2–102.3: how many opponents this player currently has; players
   * who have left the game are no longer opponents. Players are counted,
   * never the objects they control. */
  | { condition: "opponent-count"; who: PlayerRef; comparison: Comparison }
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
  /** CR 701.4b: whether an object with the quality was beheld when the action was taken. */
  | { condition: "beheld"; quality: ObjectFilter }
  /** CR 702.179e: a player has max speed if their speed is 4. */
  | { condition: "max-speed"; who: PlayerRef }
  /** CR 701.54c: how many times the Ring has tempted the player this game. */
  | { condition: "ring-tempted"; who: PlayerRef; comparison: Comparison }
  /** CR 701.54a, 701.54d: the creature `who` chose as their Ring-bearer as
   * the Ring tempted them in the event that triggered this ability matches
   * `chosen`. False when no creature was chosen. Only in the intervening-if
   * of a "ring-tempts" trigger; not whether it is the Ring-bearer now. */
  | { condition: "ring-bearer-choice"; who: PlayerRef; chosen: ObjectFilter }
  | { condition: "you-do" }
  | { condition: "you-dont" }
  /** "If you don't cast it [this way]": the card of the immediately
   * preceding cast instruction was not cast with that instruction, whether
   * its condition was false, the player declined or the card couldn't be
   * cast (CR 608.2c). `what` is the printed reference to that card. */
  | { condition: "not-cast"; who: PlayerRef; what: ObjectRef; scope: "preceding-instruction" }
  /** CR 118.12, about the immediately preceding instruction. "unable": the
   * player could not begin its mandatory action; not a declined optional
   * action. "performed" / "not-performed" ("if they do" / "if that player
   * doesn't"): whether that player chose to take its optional action or
   * began its mandatory one, regardless of what events then occurred. For
   * you, these two are the you-do / you-dont nodes. */
  | { condition: "preceding-action"; who: PlayerRef; outcome: "unable" | "performed" | "not-performed" }
  /** multiplier ("pays {1} for each card in your graveyard"): the payment
   * is the printed mana that many times, as one total cost determined when
   * the payment is offered (CR 118.12a, 608.2h); zero units make it {0}. */
  | { condition: "pays"; who: PlayerRef; cost: Cost[]; multiplier?: Amount }
  /** CR 118.12a: the alternative of an "unless" instruction when it is
   * damage rather than a cost: `who` may have `source` deal that much damage
   * to themself. True if that player chose to; the damage is dealt by
   * `source` in the usual way and can be prevented or replaced. */
  | { condition: "has-damage-dealt"; who: PlayerRef; source: ObjectRef; amount: Amount }
  | { condition: "status"; what: ObjectRef; status: string; time?: "previous" }
  | { condition: "is-filter"; what: ObjectRef; filter: ObjectFilter; time?: "previous" }
  | { condition: "count"; of: ObjectFilter; comparison: Comparison }
  /** The number of objects in `among` that are `color` is at least
   * ("included": "or is tied for most common") or strictly greater than
   * ("excluded": "but isn't tied …") the number that are each other color.
   * An object counts toward each of its colors (CR 105.2). */
  | { condition: "most-common-color"; color: Color; among: ObjectRef; ties: "included" | "excluded" }
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
  /** CR 404.2: a graveyard keeps its order. `what` is in that one
   * graveyard and the cards above it there satisfy `above`: "directly", the
   * card immediately above it matches; "count", the number of matching cards
   * anywhere above it is compared. False when `what` is not in it. */
  | { condition: "graveyard-position"; what: ObjectRef; graveyard: ZoneRef;
      above: { match: "directly"; filter: ObjectFilter } | { match: "count"; filter: ObjectFilter; comparison: Comparison } }
  | { condition: "cards-in-hand"; who: PlayerRef; comparison: Comparison };

export type PlayerDesignation = "monarch" | "initiative" | "city's-blessing"
  /** CR 702.195b: kept for the rest of the game; any number of players. */
  | "enduring-story";

export type LinkedChoiceKeyword = "bargain" | "gift" | "collect-evidence" | "tribute" | "escape"
  | "prowl" | "surge" | "madness" | "spectacle" | "emerge" | "freerunning" | "mayhem" | "sneak" | "teamwork";

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
  /** CR 602.2: activating a matching ability (counted per activation);
   * mana abilities included unless the filter excludes them (605.3a). */
  | { action: "activate"; what: ObjectFilter }
  | { action: "draw" | "play-land" | "commit-crime" | "scry" | "surveil" }
  /** CR 122.6: counters this player put on a matching recipient, including
   * counters a permanent was given as it entered. The comparison counts
   * counters; a filter recipient is matched as each counter was put. */
  | { action: "put-counters"; counter: CounterSpec | "any";
      on: { match: "object"; what: ObjectRef } | { match: "filter"; filter: ObjectFilter } };

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
  /** CR 611.2a: stated "for the rest of the game", or the lifetime of a
   * lasting permission printed with no duration; it never expires. */
  | { duration: "rest-of-game" }
  /** Ends when the referenced card is cast from that zone (CR 611.2a). */
  | { duration: "until-cast"; what: ObjectRef; from: ZoneRef }
  /** "until you exile another card with [this permanent]": ends the next
   * time that player exiles a card with that object (CR 607.2a, 611.2a). */
  | { duration: "until-next-linked-exile"; who: PlayerRef; source: ObjectRef }
  | { duration: "your-next-turn" }
  /** The rule applies only while that player's next turn is taking place
   * ("during their next turn"), not until it begins. */
  | { duration: "during-next-turn-of"; who: PlayerRef }
  /** Likewise limited to that player's next combat phase ("attacks during
   * its controller's next combat phase if able"), not the whole turn. */
  | { duration: "during-next-combat-of"; who: PlayerRef };

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
export type ChosenValueCategory = "color" | "card-type" | "card-name" | "player" | "parity";
/** CR 607.2d: refers to the linked choice, not a fresh selection or a literal
 * value. Binding must identify the originating ability/choice instance. */
export interface ChosenValueRef<Category extends ChosenValueCategory = ChosenValueCategory> {
  choice: "chosen-value";
  category: Category;
  which: "chosen" | "last-chosen";
}

export type ValueChoice =
  /** `among`: "a color of a permanent you control" — only a color that some
   * matching object is; with none, there is no color to choose (CR 106.5). */
  | { category: "color"; count?: number; excluding?: Color[]; options?: Color[]; among?: ObjectFilter }
  | { category: "card-type"; excluding?: string[] }
  /** CR 205.3e: excluding lists creature types that can't be chosen. */
  | { category: "creature-type"; excluding?: string[] }
  | { category: "basic-land-type" | "land-type" }
  /** An integer (CR 107.1). `minimum`/`maximum` are inclusive bounds:
   * "between 0 and 13" (as in read ahead's "between one and …", CR 714.3b),
   * "greater than 0" (minimum 1). */
  | { category: "number"; minimum?: number; maximum?: number }
  /** "Choose odd or even": one of the two values of a number's parity. */
  | { category: "parity" }
  /** CR 201.4/201.4a: the name of a card in the Oracle card reference. With
   * filter, only names of cards whose Oracle characteristics match (per
   * 201.4b–f for multi-part cards); excluding removes the names of matching
   * cards ("other than a basic land card name"). */
  | { category: "card-name"; filter?: ObjectFilter; excluding?: ObjectFilter }
  | { category: "type"; options: { type: "card-type" | "subtype"; value: string }[] };

export type PlayerRef =
  | { player: "prevention-event-player"; role: "recipient" }
  | { player: "you" }
  /** CR 102.4: "your team" is you and/or your teammates; in a game that
   * isn't a multiplayer game between teams it means the same as you. */
  | { player: "your-team" }
  | { player: "a-player" }
  /** The same selection/quantifier with additional eligibility. A target in
   * base remains a target; this wrapper does not create a resolution choice. */
  | { player: "filtered"; base: PlayerRef; filter: PlayerFilter }
  | { player: "replacement-event-player"; role: "actor" | "recipient" }
  | { player: "each-player" }
  | { player: "each-opponent" }
  /** `count`/`upTo` ("any number of target players", "up to two target
   * opponents"): one target word declares that many different players
   * (CR 115.3, 601.2c); each of them is affected. Absent count is one.
   * `random` ("target opponent chosen at random"): the game, not the
   * controller, chooses which legal player becomes the target as the spell
   * or ability is put on the stack (CR 601.2c); it is still a target. */
  | ({ player: "target-player"; count?: Amount; upTo?: true; random?: true } & TargetSelection)
  | ({ player: "target-opponent"; count?: Amount; upTo?: true; random?: true } & TargetSelection)
  | { player: "an-opponent" }
  /** "Any opponent may …", "any other player may …": one optional
   * instruction offered to the players of `among` (a quantified reference),
   * which at most one of them performs — a player who chooses to. Neither
   * each of them performing their own action ("each opponent may") nor one
   * opponent picked by you ("an opponent"). The printed follow-ups "If a
   * player does" / "If no player does" ask whether one of them did. */
  | { player: "any-one-of"; among: PlayerRef }
  | { player: "that-player" }
  /** "Those players": the plural counterpart of "that player", the players an
   * earlier instruction or event identified; never a new choice (CR 608.2c). */
  | { player: "those-players" }
  /** "That player or that planeswalker's controller": one earlier
   * object-or-player antecedent. This is that antecedent if it is a player,
   * otherwise the controller of that object; exactly one case applies. It is
   * neither a choice nor a pair of players (CR 608.2c). */
  | { player: "that-player-or-controller"; of: ObjectRef }
  /** Definite contextual reference, not a new player choice. Its original
   * event/choice role must be established before execution. */
  | { player: "the-player" }
  /** CR 725.1: the player who currently has the monarch designation. */
  | { player: "the-monarch" }
  /** A definite description of one player among all players: "the player
   * with the most life", "the player who controls the most creatures". It
   * denotes the single player matching `filter` when the instruction is
   * carried out, and no player when none or several do; it chooses nobody
   * (a printed tie-break would be a separate instruction). */
  | { player: "definite"; filter: PlayerFilter }
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
  | { player: "defending-player" }
  /** CR 506.2: during the combat phase the active player is the attacking
   * player. "The/that attacking player" is that player, wherever the
   * wording appears in a combat-related trigger or effect. */
  | { player: "attacking-player" };

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
  /** The candidate gained or lost any life during the period (CR 119.3):
   * at least one such event, of any size and from any cause, damage and
   * life payments included; a net change of life total is irrelevant. */
  lifeChange?: { change: "gained" | "lost"; during: "this-turn" };
  otherThan?: PlayerRef;
  /** Unresolved lexical "other" ("each other player", "each other
   * opponent"): every candidate except one contrasted player the wording
   * leaves to context — the ability's controller, or a player the ability
   * named before ("Whenever a player casts …, each other player draws").
   * Like ObjectFilter.other, never assumed to be you by default. */
  other?: true;
  /** "Each opponent attacking that player": the candidate is the attacking
   * player (CR 506.2) of one or more creatures attacking the referenced
   * player (CR 508.1b, 506.3). */
  attackingPlayer?: PlayerRef;
  /** "Each opponent who does / doesn't / can't": what each candidate did
   * with their own part of the directly preceding instruction, with the
   * outcomes of the preceding-action condition (CR 118.12). */
  precedingAction?: "performed" | "not-performed" | "unable";
  /** "Who discarded a card this way", "who didn't sacrifice a permanent this
   * way": the number of matching objects each candidate did the event to.
   * `of` carries the event-result relation without an actor; the candidate
   * is its actor. A negated existential is a count of zero. */
  eventResults?: { of: ObjectFilter; comparison: Comparison };
  /** "Each friend" / "each foe": the candidate received this label from the
   * preceding choose-friend-or-foe instruction of the same ability. */
  friendOrFoe?: "friend" | "foe";
  /** "Who searched their library this way": the candidate searched that zone
   * (`their`: the candidate's own) in the search instruction earlier in this
   * resolution, whether or not they found a card (CR 701.23a-b); the player
   * counterpart of the searched-zone condition. */
  searched?: { zone: ZoneRef; scope: "this-way" };
  /** "Who voted for a choice you voted for / didn't vote for": in the vote
   * just finished (CR 701.38a), the candidate voted for at least one choice
   * that `as` did (`same`) or did not (`!same`) vote for. */
  votedFor?: { as: PlayerRef; same: boolean };
}

/** Conjunctive restrictions on the complete selected group. These never
 * restrict a single candidate independently. Different names uses CR201.2b:
 * every object has at least one name and no pair shares a name. Sums include
 * signed values (CR107.1b); zero selected objects have an empty sum of zero. */
export type GroupConstraint =
  | { constraint: "shared-value"; characteristic: SharedCharacteristic }
  | { constraint: "attribute-sum"; attribute: "power" | "toughness" | "mana-value"; comparison: Comparison }
  /** "controlled by the same player": one player controls every selected
   * object. With "controlled by different players" (pairwise-different
   * "controller") no player controls two of them (CR 108.4). */
  | { constraint: "same-controller" }
  | { constraint: "pairwise-different"; property: "name" | "power" | "mana-value" | "controller" };

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
  /** CR 404.1–404.2: a graveyard is an ordered pile with the most recently
   * added card on top. The `count` cards nearest the top; with `matching`,
   * the nearest that match it, passing over the others ("the top creature
   * card"). Determined by the pile's order, never chosen; fewer may exist. */
  | { ref: "graveyard-cards"; position: "top"; count: Amount; matching?: ObjectFilter; of: ZoneRef }
  /** CR 700.3b: every object in one pile of the preceding separate-piles.
   * "chosen" is the pile picked by the preceding choose-pile; "choice" is a
   * pile `who` picks as the enclosing instruction is carried out ("put one
   * pile …", "the pile of their choice"). The objects of the other pile are
   * the `rest` of the separated objects. */
  | ({ ref: "pile" } & ({ pile: "chosen" } | { pile: "choice"; who: PlayerRef }))
  | { ref: "this"; noun: string }                            // "this creature", "this spell"
  | { ref: "that"; filter: ObjectFilter; plural?: true }      // "that card", "those creatures"
  /** Select the stated number from an established pool at the enclosing
   * instruction's choice time (payment or resolution), not a new target.
   * As a trigger's subject ("whenever either of those creatures deals …") it
   * matches any one member of the pool that performs the event, like "a
   * creature you control" with count 1. */
  | { ref: "from-pool"; pool: ObjectRef; count: Amount; upTo?: true; groupConstraints?: GroupConstraint[]; random?: true }
  /** Distinct card allocations for search/reveal/move instructions, e.g.
   * "a creature card and/or an enchantment card". This is not a filter OR
   * under one count, nor reusable representative roles (Tragic Arrogance).
   * The chosen members' counts remain independent of the outer may choice.
   * A shared source applies to every member. CR608.2c/d, 701.23b; Kaalia's
   * M20 ruling assigns a multitype card to just one extraction slot. */
  | { ref: "card-extraction-group"; members: CardExtractionSlot[]; combine: "all" | "any-nonempty"; allocation: "distinct-objects";
      source?: { kind: "pool"; pool: ObjectRef } | { kind: "zone"; zone: ZoneRef } }
  /** `chosenBy` ("target creature of an opponent's choice", "of their
   * choice"): that player, not the controller, chooses this target as the
   * spell or ability is put on the stack (CR 601.2c, 603.3d); it is still a
   * target of that spell or ability. "their" stays the pronoun `they`. */
  | ({ ref: "target"; filter: ObjectFilter; count: Amount; upTo?: boolean; groupConstraints?: GroupConstraint[]; random?: true; chosenBy?: PlayerRef } & TargetSelection)
  /** CR 115.4: "any target", "another target", "two targets". `other` is the
   * unresolved lexical contrast, exactly as ObjectFilter.other. */
  /** `excluding` ("any target that isn't a Dragon"): an object matching it
   * isn't a legal target; players never match an object filter. */
  | ({ ref: "any-target"; count?: Amount; upTo?: boolean; other?: true; random?: true; chosenBy?: PlayerRef; excluding?: ObjectFilter } & TargetSelection)
  /** The single object `source` targets, as announced (CR 601.2c): "the
   * creature it targets" in a cost change of that spell or ability, whose
   * targets are chosen before its total cost is determined (601.2f).
   * `affected-object` is each spell or ability of an adjusted group. */
  | { ref: "targeted-by"; source: ObjectRef | { ref: "affected-object" }; filter: ObjectFilter }
  /** Symbolic event/linked referent; antecedent resolution is a later pass. */
  | { ref: "related"; filter: ObjectFilter; relation: ObjectRelation; number: "singular" | "plural" }
  /** CR 702.95b: both members of a soulbond pair — `of` and the creature
   * it is paired with. Denotes nothing while `of` is unpaired. */
  | { ref: "soulbond-pair"; of: ObjectRef }
  /** Complement of the selected objects in the contextual or stated pool. */
  | { ref: "rest"; of?: ObjectRef }
  /** All cards in the named player's personal zone, not the zone itself. */
  | { ref: "zone-cards"; zone: "graveyard" | "hand" | "library"; owner: PlayerRef }
  /** CR 702.124e: if both commanders qualify, choice occurs as the effect applies. */
  | { ref: "commander"; owner: PlayerRef; count: Amount }
  | { ref: "each"; filter: ObjectFilter }
  /** CR 701.38: the objects that received the most votes in the preceding
   * vote, including objects tied for the most. */
  /** CR 701.47c: "the Army you amassed" / "the amassed Army" is the Army
   * creature chosen by the preceding amass, whether or not it received counters. */
  | { ref: "amassed-army" }
  | { ref: "vote-leaders"; filter: ObjectFilter }
  | { ref: "all"; filter: ObjectFilter }
  /** `random`: the game selects the stated number at random from the matching
   * objects ("a card at random from your hand"); no player chooses. */
  | { ref: "filter"; filter: ObjectFilter; count: Amount; upTo?: boolean; groupConstraints?: GroupConstraint[]; random?: true } // "a creature", "up to seven lands"
  /** CR 702.55b: the object targeted by this card's haunt ability ("the
   * creature it haunts", "the creature this card haunts"), whether or not it
   * still is a creature; never a description of haunted creatures in general. */
  | { ref: "haunted-creature" }
  | { ref: "equipped" | "enchanted"; noun: string }         // "equipped creature"
  /** "fortified land": the land this Fortification is attached to (CR 301.6, 301.5f). */
  | { ref: "fortified"; noun: "land" };

/**
 * Structured card/object filter. All fields and ordinary array entries are
 * conjunctive. Explicit anyOf branches represent alternatives.
 */
export type ObjectRelation = {
  /** Event time, distinct from an enclosing effect or permission duration. */
  period?: "this-turn";
  who?: PlayerRef;
} & (
  /** What the named event actually did, after replacement and impossibility
   * (CR 608.2c, 609.3, 614.6). The enclosing filter describes each object as
   * that event happened to it, not its current state (608.2h–i). "die" is
   * put into a graveyard from the battlefield (700.4). "cast" with "this-way":
   * a spell cast using the cast permission or instruction printed just
   * before, for as long as it is that spell.
   * "choose" ("each permanent chosen this way", only with "this-way"): the
   * objects the preceding "choose" instructions of this ability selected
   * (resolution choices, CR 608.2d, or "choose target …" declarations,
   * 601.2c), by whichever players chose; never a fresh choice. */
  | { relation: "event-result"; event: "exile" | "sacrifice" | "discard" | "reveal" | "destroy"
      | "tap" | "return" | "counter" | "mill" | "draw" | "die" | "cast" | "choose"; scope: "contextual" | "this-way" }
  /** A completed move into the stated zone ("put into your graveyard from
   * the battlefield this turn", "returned to your hand this way", "exiled
   * from their hand this way": to exile, with that origin). Each move
   * is a separate occurrence of a new object (CR 400.7); the filter describes
   * it as it moved, wherever it is now. An origin is recorded only when
   * printed. "this-way" limits it to moves made by the same resolving
   * instruction sequence; otherwise `period` is the window. */
  | { relation: "zone-change"; to: ZoneRef; from?: ZoneRef | "anywhere"; scope?: "this-way" }
  | { relation: "linked-exile"; source: ObjectRef }
  /** CR 702.167c: "exiled cards used to craft it" — the cards exiled as the
   * craft cost of `of`'s ability was paid (the materials). */
  | { relation: "craft-material"; of: ObjectRef }
  /** CR 607.2c: "created with [this object]" — a token created as a result
   * of the linked ability printed on `source`, never another token that
   * object or its controller made. */
  | { relation: "linked-creation"; source: ObjectRef }
  /** "That had a [kind] counter put on it this way" (CR 122.1, 608.2c): the
   * object had at least one counter of that kind ("any": of any kind) put
   * on it by the earlier instruction of this resolution. */
  | { relation: "counters-put"; counter: CounterSpec | "any"; scope: "this-way" }
  /** CR 702.82b: "it devoured" — sacrificed as a result of `by`'s devour
   * ability as `by` entered the battlefield. The enclosing filter describes
   * each permanent as it was sacrificed (608.2h). */
  | { relation: "devoured"; by: ObjectRef }
  /** The object was tapped to pay the cost of a crew ability of `of` (it
   * crewed that Vehicle, CR 702.122b-c), of a saddle ability of `of`
   * (702.171c), or for mana in the total cost of the spell `of` by convoke
   * (702.51c; for a permanent, the spell it was). `period` limits crewing and
   * saddling to this turn's activations. Neither the tapped creature's
   * current state nor whether the ability resolved matters. */
  | { relation: "tapped-for-cost"; action: "crew" | "saddle" | "convoke"; of: ObjectRef }
);

/** Characteristic-set intersections, never spelling comparisons or a count
 * of printed strings. Multiple names follow CR201.2a/c; colorless objects
 * share no color. Creature/land types remain distinct subtype categories. */
export type SharedCharacteristic = "name" | "color" | "card-type" | "creature-type" | "land-type"
  /** CR 110.4: one of the six permanent types (artifact, battle, creature, enchantment, land, planeswalker). */
  | "permanent-type";
export type CharacteristicOperand =
  | { operand: "object"; what: ObjectRef }
  | { operand: "matching-set"; filter: ObjectFilter; quantifier: "any" | "every" };
export type CharacteristicRelation =
  | { relation: "shared-value"; characteristic: SharedCharacteristic; with: CharacteristicOperand;
      /** "shares one of those types with it": only the card types listed in
       * the nearest preceding type list count as shared; that list is not
       * resolved here, like a pronoun's antecedent. */
      among?: "those-types" }
  /** Candidate must have a name and share none with the compared object(s).
   * This is NOT the negation of shared-name for a nameless candidate. */
  | { relation: "different-name"; from: CharacteristicOperand }
  | { relation: "not"; of: CharacteristicRelation };

export interface ObjectFilter {
  characteristicRelations?: CharacteristicRelation[];
  allOf?: ObjectFilter[];
  damageHistory?: DamageHistoryPredicate;
  /** The object itself made this zone change in the window; the fields are
   * those of the zone-change-history condition, and the other parts of the
   * filter and `controllerAtMove` describe it as it moved (CR 608.2i).
   * "Card in your graveyard that was put there …" repeats that zone as `to`.
   * The predicate does not say where the object is now: a selection can only
   * take objects that still exist as the same object (400.7), while a count
   * ("for each creature that died this turn") looks back at every one. */
  zoneChangeHistory?: { event: "dies" | "enters" | "put-into-zone"; from?: ZoneRef | "anywhere"; to?: ZoneRef;
    controllerAtMove?: PlayerRef; during: "this-turn" };
  anyOf?: ObjectFilter[];
  /** Printed "except for <A>, <B>, and <C>": the object matches none of
   * these descriptions, each read in the enclosing instruction's context.
   * An exception that is one card type, one subtype, tokens or the source
   * itself is recorded as nonTypes / nonSubtypes / nonToken / otherThan. */
  noneOf?: ObjectFilter[];
  /** All listed card types: "artifact creature". */
  types?: string[];
  /** "noncreature", "nonland" … */
  nonTypes?: string[];
  /** Object class this filter selects over. */
  cls?: "permanent" | "card" | "spell" | "token" | "ability" | "source";
  /** Ability identity is separate from its source object's characteristics. */
  abilityKind?: "activated" | "triggered" | "static" | "spell";
  manaAbility?: boolean;
  /** "Abilities with {T} in their costs": the ability's cost includes the tap symbol (CR 107.5). */
  tapInCost?: true;
  loyaltyAbility?: boolean;
  /** "with {T} in its activation cost" (true) / "without {T} in its activation
   * cost" (false): whether the ability's activation cost includes {T} (CR 107.5). */
  tapSymbolInCost?: boolean;
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
  /** CR 701.54b: has the Ring-bearer designation (not a copiable value). */
  ringBearer?: true;
  /** Currently has at least one sticker of this kind on it; "any" is a
   * "stickered" object (CR 123.4). */
  sticker?: StickerKind;
  subtypes?: string[];
  nonSubtypes?: string[];
  supertypes?: string[];
  nonSupertypes?: string[];
  colors?: Color[];
  nonColors?: Color[];
  /** false: "one or more colors", any object that isn't colorless (CR 105.2). */
  colorless?: boolean;
  multicolored?: boolean;
  monocolored?: boolean;
  nonToken?: boolean;
  /** "attacking-alone"/"blocking-alone" are the CR 506.5 states: attacking
   * or blocking while no other creature is. */
  status?: string[];                                         // tapped, attacking …
  /** "that isn't enchanted": has none of these statuses. Only statuses
   * without a printed antonym ("isn't tapped" is status untapped). */
  nonStatus?: string[];
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
  /** CR 715.1: the card has an Adventure ("a creature spell that has an Adventure"). */
  adventure?: true;
  control?: "you" | "opponent" | "not-you" | "that-player";
  /** A specifically referenced controller, including a targeted player. */
  controlledBy?: PlayerRef;
  /** Historical/proposed casting action, separate from control/current zone. */
  cast?: { by: PlayerRef; from?: ZoneRef };
  /** CR 702.185c: the spell was cast for the alternative cost of this
   * keyword ("a spell was warped this turn"). */
  castForKeywordCost?: "warp";
  /** "that weren't cast this way" / "not cast this way": of the described
   * cards, only those not cast with the cast instruction or permission
   * earlier in this same resolving sequence (CR 608.2c). */
  notCastThisWay?: true;
  /** Literal name is unchanged when the source's name changes (CR 201.5). */
  name?: { name: "literal"; value: string };
  /** "not named X": none of the object's names is this literal name (CR
   * 201.2); an object with no name qualifies. */
  notName?: { name: "literal"; value: string };
  /** Symbolic prior type choice (CR 607.2d). Bare "chosen type" does not
   * determine whether that choice was a card type or a subtype. */
  chosenType?: { choice: "chosen-type"; kind?: "card" | "creature" | "land" | "artifact" | "enchantment" | "planeswalker" };
  /** "that isn't of the chosen type": lacks the linked chosen type. */
  notChosenType?: { choice: "chosen-type"; kind?: "card" | "creature" | "land" | "artifact" | "enchantment" | "planeswalker" };
  chosenColor?: ChosenValueRef<"color">;
  /** "of the creature type / color of your choice": one value that `who`
   * announces while the instruction containing this filter is applied
   * (CR 608.2d); every object tested by the filter uses that same value.
   * Not a linked earlier choice (chosenType / chosenColor). Never used in
   * a target description, whose legality is needed before resolution. */
  valueOfChoice?: { choice: "fresh-choice"; category: "color" | "creature-type"; who: PlayerRef; when: "instruction-applies" };
  /** Has the chosen card name among its names (CR 201.2a, 201.4g); the
   * symbolic linked choice of CR 607.2d, never a literal name. */
  chosenName?: ChosenValueRef<"card-name">;
  own?: "you" | "opponent" | "not-you";
  /** "with mana value of the chosen quality", "with even mana values": the
   * attribute is an odd or an even number (0 is even), printed or the
   * linked odd-or-even choice (CR 607.2d). */
  parity?: { property: "power" | "mana-value"; parity: "odd" | "even" | ChosenValueRef<"parity"> };
  /** A specifically referenced owner ("that player owns", "target player
   * owns"), as controlledBy is for control. Ownership never changes with
   * control or zone (CR 108.3). */
  ownedBy?: PlayerRef;
  zone?: ZoneRef;
  /** CR 400.11: in none of the game's zones (a sideboard, 400.11a). Outside
   * the game is not a zone, so this is deliberately not a ZoneRef. */
  outsideGame?: true;
  /** CR 905.1d, 100.4: a card of this player's card pool (the cards they
   * drafted) that they did not put in their deck. */
  draftedNotInDeck?: { drafter: PlayerRef };
  /** In any zone other than this one ("that aren't on the battlefield"). */
  outsideZone?: ZoneRef;
  properties?: PropertyConstraint[];
  /** "with greater power", "with lesser mana value": strictly greater (gt)
   * or less (lt) — "with equal or lesser/greater …": at most (le) or at
   * least (ge) — than the same attribute of a standard the text leaves
   * unstated — the creature a blocking restriction applies to, the spell
   * that was cast, the creature that died … (CR 208.1, 202.3). That standard
   * is contextual and must be bound; it is never assumed to be the source. */
  implicitComparison?: { attribute: "power" | "toughness" | "mana-value"; op: "gt" | "lt" | "ge" | "le" };
  /** "with the same controller" (Bioshift, Fear, Fire, Foes!): controlled by
   * the same player as a standard the text leaves unstated — an object named
   * earlier in the same instruction ("target creature"). Like
   * implicitComparison, that standard is contextual, must be bound, and is
   * never assumed to be the source (CR 108.4, 110.2). */
  implicitSameController?: true;
  /** "that targets …" */
  targets?: ObjectRef | PlayerRef;
  /** "that targets only …" (CR 115.9c): the different objects and players
   * chosen as its targets number exactly one, and that one is this operand
   * (membership for a query operand). */
  targetsOnly?: ObjectRef | PlayerRef;
  /** Printed "with a single target": the spell or ability has exactly one
   * target, whatever that target is. */
  singleTarget?: true;
  /** "…or player": the filter unions in players ("target creature or player"). */
  orPlayer?: boolean;
  /** "…or opponent": unions in only the opponents of the spell or ability's
   * controller ("target opponent or planeswalker"). The object branch is not
   * restricted by controller (CR 102.2–3, 115.1). Never set with orPlayer. */
  orOpponent?: boolean;
  /** Validated canonical keyword names; every listed ability is required.
   * A specific landwalk is its printed word ("islandwalk", CR 702.14a). */
  withAbility?: string[];
  /** Every listed keyword must be absent. */
  withoutAbility?: string[];
  /** false: "with no abilities", the object has no abilities at all (CR
   * 113.12 example): characteristics set or qualities stated by effects are
   * not abilities, abilities granted by effects are. */
  hasAbilities?: false;
  /** Conjunctive counter-count requirements on the selected object itself. */
  counterConstraints?: { counter: CounterSpec | "any"; comparison: Comparison }[];
  relation?: ObjectRelation;
  /** Selection is limited to this previously established pool of objects. */
  fromAmong?: ObjectRef;
}

export interface PropertyConstraint {
  /** Base values ignore modifying effects and counters (CR 208.4b). */
  property: "power" | "toughness" | "mana-value" | "base-power" | "base-toughness";
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
   * heuristic supplies this contextual reference. "difference" is the
   * difference between the two results of one two-dice roll. */
  | { amount: "dice-result"; roll: { roll: "contextual" }; measure: "result" | "total" | "difference" }
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
  /** CR 709.5c, 709.5j: halves with an unlocked designation among the Room
   * permanents matched by `among`. */
  | { amount: "unlocked-door-count"; among: ObjectFilter }
  /** CR 601.2c, 601.2f: the number of targets chosen for the spell or ability
   * whose total cost is being determined; that number never changes once the
   * targets are announced. `beyond` leaves that many targets uncounted ("for
   * each target beyond the first" is 1). "affected-object" is each spell or
   * ability the enclosing cost modification applies to. `matching` counts
   * only the targets that are objects of that description ("for each
   * creature it targets"); a target that is not one adds nothing. */
  | { amount: "target-count"; of: ObjectRef | { ref: "affected-object" }; beyond?: number; matching?: ObjectFilter }
  /** How much `who` did over the period: the objects (sacrifice, discard,
   * draw, create-token, cycle) or occurrences that the player-action-history
   * condition counts, each as the action happened. Several players' actions
   * are added together; "all-players-combined" is the passive wording that
   * names no player. */
  | { amount: "player-action-count"; who: PlayerRef | "all-players-combined"; action: PlayerHistoryAction; during: "this-turn" }
  /** Matching spells cast over the period (CR 601.2i; copies are not cast),
   * described as they were cast. Several players' spells are added together. */
  | { amount: "cast-count"; who: PlayerRef | "all-players-combined"; what: ObjectFilter; during: "this-turn" | "this-game";
      /** The zone the spells were cast from (a commander cast from the command zone, CR 903.8). */
      from?: ZoneRef }
  /** CR 107.14: energy counters `who` has paid (as a cost) or lost (to any
   * effect) over the period, counted per {E}. */
  | { amount: "energy-paid-or-lost"; who: PlayerRef; during: "this-turn" }
  /** CR 903.8: how many times that one object (a commander) has been cast
   * from that zone this game, as the commander tax counts it per card.
   * "affected-object" is each spell the enclosing cost modification adjusts. */
  | { amount: "times-cast"; what: ObjectRef | { ref: "affected-object" }; from: ZoneRef; during: "this-game" }
  /** CR 700.8a–b: the number of creatures in that player's party, computed
   * by the game to the highest result from zero to four. */
  | { amount: "party-size"; who: PlayerRef }
  /** CR 102.2–102.3: how many opponents this player currently has. Players
   * are counted, never the objects they control; a player who has left the
   * game is no longer an opponent. */
  | { amount: "opponent-count"; who: PlayerRef }
  /** CR 506.3: only a player, planeswalker or battle can be attacked. Counts
   * players, never creatures: those currently being attacked in the printed
   * combat, or those `by` declared an attack against in the window. With
   * "opponents", only opponents of the source's controller (resp. of `by`)
   * are counted (CR 102.2). `by` with a status limits the current attackers
   * to that player's creatures ("opponents you're attacking"). Whether an attack on a player's planeswalker or
   * battle makes that player counted is not stated by the printed text. */
  | { amount: "attacked-player-count"; players: "players" | "opponents";
      which: { status: "being-attacked"; by?: PlayerRef } | { by: PlayerRef; during: "this-turn" | "this-combat" } }
  /** How many players the quantified reference ranges over: "the number of
   * opponents who control more lands than you", "for each player who lost
   * life this turn". Each qualifying player counts once, whatever the size
   * of what they did or control; players who left the game don't count
   * (CR 102.2-102.3). `of` is a quantified reference (each player / each
   * opponent, with its eligibility filter), never a selection or a target. */
  | { amount: "player-count"; of: PlayerRef }
  /** Distinct creature incarnations declared as attackers in this window,
   * across all players. Repeated attacks by the same incarnation count once;
   * leaving the battlefield or ceasing to be a creature does not erase the
   * recorded attack. Entering attacking is excluded (CR400.7,508.4). This is
   * a historical set cardinality, not a query of current battlefield objects. */
  | { amount: "combat-history-count"; action: "attacked"; during: "this-turn" | "this-combat"; distinct: "object-incarnations";
      /** Only attackers declared by this player ("creatures you attacked with"). */
      by?: PlayerRef }
  /** Cardinality of the UNION of represented values, not number of objects.
   * Basic land types are Plains/Island/Swamp/Mountain/Forest, regardless of
   * whether a qualifying land has the Basic supertype (CR 305.6). An artifact
   * creature contributes two card types; repeated values count once (205.2b).
   * Colorless contributes no color (105.2c). Every object has exactly one
   * mana value, 0 when it has no mana cost (202.3, 202.3a). Power is each
   * object's current power. For "name" ("differently named") this is the
   * largest number of those objects that have different names under
   * CR 201.2b; an object with no name counts for none. For "counter-kind"
   * ("kinds of counters among permanents you control") each kind of counter
   * on any of those objects counts once, whatever their number (CR 122.1). */
  | { amount: "distinct-count"; characteristic: "card-type" | "permanent-type" | "creature-type" | "basic-land-type" | "color" | "mana-value" | "power" | "name" | "counter-kind"; among: ObjectRef }
  /** Actual mana paid to cast the referenced spell, including additional
   * costs, not its printed mana cost/value (601.2f/h, 609.4b). A permanent may
   * refer to its originating spell's payment history under CR 400.7d.
   * "snow" counts the mana produced by snow sources among it (CR 107.4h). */
  | { amount: "mana-spent"; what: ObjectRef; quantity: "total" | "distinct-colors" | "snow";
      /** Only mana produced by a source matching this filter counts (CR 106.3), as in "each mana from a Treasure". */
      source?: ObjectFilter }
  /** CR 607.2g: the life paid as `of` entered the battlefield, by the linked
   * "As this … enters, pay … life" ability printed on it; no other payment. */
  | { amount: "entry-payment"; of: ObjectRef; paid: "life" }
  | { amount: "attribute-sum"; attribute: "power" | "toughness" | "mana-value"; among: ObjectRef }
  /** Multiplies a quantity inside one instruction/event; this is not a loop
   * that performs the instruction as separate events. */
  | { amount: "product"; factors: Amount[] }
  /** `total`: one sum over every player `on` denotes ("poison counters your
   * opponents have" is collective possession, CR 122.1), never a per-player
   * value. */
  | { amount: "counter-count"; counter: CounterSpec | "any"; on: ObjectRef | PlayerRef; total?: true }
  /** Number of times the originating spell's kicker/multikicker was paid,
   * retained across its move to the battlefield (CR 400.7d, 702.33). */
  | { amount: "kicker-count"; of: ObjectRef }
  /** Number of times a mutating creature spell has merged with this
   * permanent since it became a new object (CR 702.140c-d, 400.7). */
  | { amount: "mutation-count"; of: ObjectRef }
  | { amount: "attribute-extremum"; extremum: "maximum" | "minimum"; attribute: "power" | "toughness" | "mana-value"; among: ObjectFilter }
  /** Base values ("its base power") are set by characteristics and by effects
   * that set them, ignoring modifying effects and counters (CR 208.4b). */
  | { amount: "attribute"; of: ObjectRef; attribute: "power" | "toughness" | "mana-value" | "base-power" | "base-toughness" }
  /** CR 700.5: the number of mana symbols of the listed color(s) among the
   * mana costs of permanents `who` controls. With two colors, each symbol
   * that is either or both colors counts once (hybrid symbols). This is not
   * mana spent, mana value, or those permanents' current colors; modifiers
   * and layer timing follow CR 700.5a. */
  | { amount: "devotion"; who: PlayerRef; colors: [Color] | [Color, Color] | ChosenValueRef<"color"> }
  /** "The number of red mana symbols in the mana costs of permanents you
   * control": every symbol of that color in the matching objects' mana costs,
   * hybrid and Phyrexian symbols included (CR 107.4e–f, 202.2). Not devotion:
   * effects that modify devotion (700.5a) do not change it. */
  | { amount: "mana-symbols"; color: Color; in: ObjectFilter }
  | { amount: "life-total"; of: PlayerRef }
  /** The greatest or least current life total among the stated players: one
   * number, whichever players are tied for it (CR 119.1). */
  | { amount: "life-total-extremum"; extremum: "maximum" | "minimum"; among: PlayerRef }
  /** The greatest value of one count among the stated players, measured
   * separately for each of them: one number, whichever players are tied for
   * it. Counts of different players are never added together. */
  | { amount: "per-player-extremum"; extremum: "maximum"; among: PlayerRef; count: PerPlayerCount }
  /** CR 103.4: fixed when the game begins; unaffected by later life changes. */
  | { amount: "starting-life-total"; of: PlayerRef }
  /** CR 103.4 / 119.1: the format-defined total the player began the game
   * with, not their current, previous or highest life total. */
  /** Gross life gained or lost over the period, summed over every such event
   * (damage and life payments are life loss, CR 119.2–4); never a net change
   * of life total. "all-players-combined" is the printed total across players.
   * A contextual player ("they", "that player") is evaluated for that player;
   * with a group ("your opponents have lost") their changes are added together. */
  | { amount: "life-change"; who: PlayerRef | "all-players-combined"; change: "gained" | "lost"; during: "this-turn" }
  | { amount: "twice"; of: Amount }
  /** `exact`: only as the operand of a comparison ("less than half your
   * starting life total"), which compares against the exact value; nothing
   * fractional is generated, so no rounding is stated (CR 107.1a). */
  | { amount: "half"; of: Amount; round: "up" | "down" | "exact" }
  | { amount: "plus"; a: Amount; b: Amount }
  | { amount: "minus"; a: Amount; b: Amount }
  | { amount: "that-number" }
  /** "the chosen number" / "the last chosen number": the value of a linked
   * number choice (CR 607.2d), not a fresh choice. */
  | { amount: "chosen-number"; which: "chosen" | "last-chosen" }
  | { amount: "that-much" };

/** A count taken for one candidate player of a per-player extremum. `of`
 * has no controller or actor of its own; the candidate supplies it. */
export type PerPlayerCount =
  /** Objects the candidate currently controls. */
  | { count: "controlled"; of: ObjectFilter }
  /** Matching cards the candidate actually discarded in the stated event of
   * this resolving instruction sequence (CR 608.2h, 701.9). */
  | { count: "event-result"; event: "discard"; scope: "this-way"; of: ObjectFilter }
  /** CR 121.1: cards the candidate has drawn during the period, wherever
   * those cards are now. */
  | { count: "cards-drawn"; during: "this-turn" };

/** What one earlier event actually did, after replacement and prevention
 * (CR 608.2h, 609.3, 119.3, 120.3), never the number its instruction stated
 * and never a current-state query. "this-way" is the stated event in the same
 * resolving instruction sequence; "contextual" has no printed scope (typically
 * the triggering event) and needs its antecedent event bound. A quantity is
 * the total over every matching occurrence and participant of that event;
 * omitted participants are unconstrained, not implied. */
export type EventQuantity = { scope: "contextual" | "this-way" } & (
  | { event: "life-change"; change: "gained" | "lost"; who?: PlayerRef }
  /** CR 107.14: the energy counters actually paid in the stated payment. */
  | { event: "energy-paid" }
  /** CR 705.2: the flips of the coin-flip instruction that `who` (the
   * flipper) won, as in "for each flip you won". */
  | { event: "coin-flip"; result: "win"; who: PlayerRef }
  /** CR 118.1: the mana actually paid in the stated payment; with `who`,
   * only what that player paid. */
  | { event: "mana-paid"; who?: PlayerRef }
  /** CR 701.22a: the cards the scry action looked at (the top N, or fewer
   * when the library has fewer), not the number a player moved. */
  | { event: "scry-looked-at" }
  /** Excess damage follows CR 120.4a / 120.10: the part beyond lethal damage
   * (a creature), loyalty (a planeswalker) or defense (a battle). */
  | { event: "damage"; measure: "dealt" | "excess"; by?: ObjectRef; to?: ObjectRef | PlayerRef }
  /** CR 122.1, 608.2h/i: the counters of this kind the stated removal
   * actually took off, summed over everything they were removed from. For
   * an activated ability, "this-way" also covers a removal made in paying
   * that ability's own costs (602.2b). Never a current counter count. */
  | { event: "counter-removal"; counter: CounterSpec | "any" }
);

export interface SignedAmount {
  sign: 1 | -1;
  amount: Amount;
}
