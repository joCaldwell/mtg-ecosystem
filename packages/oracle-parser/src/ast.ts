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

export interface ParsedLine {
  text: string;
  ok: boolean;
  ability?: Ability;
  /** Human-readable failure with token position, e.g. `expected effect at "embalm"` */
  error?: string;
}

export interface ParseCardResult {
  name: string;
  /** True only if every line parsed. */
  ok: boolean;
  lines: ParsedLine[];
  /** Present when ok — the abilities of every line, in order. */
  abilities?: Ability[];
}

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
  | AdditionalCostLine;

/** "Flying, first strike, ward {2}" */
export interface KeywordLine {
  kind: "keywords";
  keywords: KeywordInstance[];
}

/** "{T}, Sacrifice ~: Draw a card." */
export interface ActivatedAbility {
  kind: "activated";
  abilityWord?: string;
  costs: Cost[];
  effects: Sentence[];
  /** "Activate only as a sorcery." etc. — raw restriction text, parsed later. */
  restriction?: string;
}

/** "+1: …", "−X: …", "0: …" */
export interface LoyaltyAbility {
  kind: "loyalty";
  cost: { sign: 1 | -1 | 0; amount: number | "x" };
  effects: Sentence[];
}

/** "Whenever ~ attacks, draw a card." */
export interface TriggeredAbility {
  kind: "triggered";
  abilityWord?: string;
  trigger: Trigger;
  /** Intervening "if" clause: "When ~ dies, if it had a counter on it, …" */
  condition?: Condition;
  effects: Sentence[];
}

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
  /** For number-parameterized keywords: Crew 2, Toxic 1, Fading 3. */
  amount?: number | "x";
  /** Protection from …, hexproof from … */
  from?: ProtectionScope[];
  /** Enchant creature, Affinity for artifacts, …walk */
  filter?: ObjectFilter;
  /** Partner with <name> */
  pairedWith?: string;
}

export type ProtectionScope =
  | { scope: "color"; color: Color }
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
  | { cost: "energy"; amount: number }               // {E}{E}
  | { cost: "pay-life"; amount: Amount }
  | { cost: "sacrifice"; what: ObjectRef }
  | { cost: "discard"; what: ObjectRef | "hand" }
  | { cost: "exile"; what: ObjectRef }
  | { cost: "tap-objects"; what: ObjectRef }         // "Tap two untapped Allies you control"
  | { cost: "return"; what: ObjectRef; to: "hand" }
  | { cost: "remove-counters"; counter: CounterSpec; count: Amount; from: ObjectRef };

// ---------------------------------------------------------------------------
// Triggers
// ---------------------------------------------------------------------------

export type Trigger =
  | { trigger: "enters"; what: ObjectRef }
  | { trigger: "leaves"; what: ObjectRef }
  | { trigger: "dies"; what: ObjectRef }
  | { trigger: "enters-or-leaves"; what: ObjectRef }
  | { trigger: "enters-or-attacks"; what: ObjectRef }
  | { trigger: "enters-or-dies"; what: ObjectRef }
  | { trigger: "put-into-graveyard"; what: ObjectRef; from?: Zone }
  | { trigger: "cast"; who: PlayerRef; what: ObjectFilter }
  | { trigger: "attacks"; what: ObjectRef; alone?: boolean; whom?: (PlayerRef | ObjectRef)[] }
  | { trigger: "you-attack" }
  | { trigger: "blocks"; what: ObjectRef; orBecomesBlocked?: boolean }
  | { trigger: "becomes-blocked"; what: ObjectRef; by?: ObjectRef }
  | { trigger: "becomes-target"; what: ObjectRef; of: ObjectFilter }
  | { trigger: "becomes-tapped" | "becomes-untapped"; what: ObjectRef }
  | { trigger: "activate"; who: PlayerRef; what: ObjectFilter }
  | { trigger: "phase"; phase: PhaseName; whose: TurnOwner }
  | { trigger: "deals-damage"; what: ObjectRef; combat?: boolean; to?: PlayerRef | ObjectRef | "any" }
  | { trigger: "is-dealt-damage"; what: ObjectRef }
  | { trigger: "gains-life"; who: PlayerRef }
  | { trigger: "loses-life"; who: PlayerRef }
  | { trigger: "draws"; who: PlayerRef }
  | { trigger: "discards"; who: PlayerRef }
  | { trigger: "sacrifices"; who: PlayerRef; what: ObjectFilter }
  | { trigger: "scries" | "surveils"; who: PlayerRef }
  | { trigger: "taps-for-mana"; what: ObjectRef };

export type PhaseName =
  | "upkeep" | "draw-step" | "untap-step" | "combat" | "declare-attackers"
  | "declare-blockers" | "combat-damage" | "end-of-combat" | "end-step"
  | "cleanup" | "main" | "precombat-main" | "postcombat-main" | "turn";

export type TurnOwner = "your" | "each-player" | "each-opponent" | "that-player" | "active-player" | "opponent";

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
  /** "Otherwise, …" branch from a following sentence. */
  otherwise?: Effect[];
}

interface EffectBase {
  /** "You may …" */
  optional?: true;
  /** "… for each Elf you control." */
  forEach?: ObjectFilter;
  /** "… until end of turn." */
  duration?: Duration;
}

export type Effect = EffectBase &
  (
    | { effect: "draw"; who: PlayerRef; amount: Amount }
    | { effect: "damage"; source: ObjectRef; amount: Amount; to: DamageTarget }
    | { effect: "destroy"; what: ObjectRef }
    | { effect: "exile"; what: ObjectRef; from?: ZoneRef }
    | { effect: "counter"; what: ObjectRef }
    | { effect: "move-zone"; what: ObjectRef; to: ZoneRef; tapped?: boolean }
    | { effect: "return-to-hand"; what: ObjectRef }
    | { effect: "create-token"; count: Amount; token: TokenSpec }
    | { effect: "pump"; what: ObjectRef; power: SignedAmount; toughness: SignedAmount }
    | { effect: "set-pt"; what: ObjectRef; power: Amount; toughness: Amount }
    | { effect: "gain-abilities"; what: ObjectRef; abilities: GainedAbility[] }
    | { effect: "gain-life" | "lose-life"; who: PlayerRef; amount: Amount }
    | { effect: "tap" | "untap"; what: ObjectRef }
    | { effect: "sacrifice"; who: PlayerRef; what: ObjectRef }
    | { effect: "discard"; who: PlayerRef; what: ObjectRef | "hand"; random?: boolean }
    | { effect: "mill"; who: PlayerRef; amount: Amount }
    | { effect: "scry" | "surveil"; amount: Amount }
    | { effect: "put-counters"; counter: CounterSpec; count: Amount; on: ObjectRef }
    | { effect: "remove-counters"; counter: CounterSpec; count: Amount | "all"; from: ObjectRef }
    | { effect: "add-mana"; mana: ManaProduction }
    | { effect: "search"; who: PlayerRef; zone: Zone; for: ObjectRef }
    | { effect: "shuffle"; who: PlayerRef }
    | { effect: "gain-control"; who: PlayerRef; what: ObjectRef }
    | { effect: "reveal"; who: PlayerRef; what: ObjectRef | "hand" }
    | { effect: "look-at"; who: PlayerRef; what: ObjectRef | LookTop }
    | { effect: "fight"; a: ObjectRef; b: ObjectRef }
    | { effect: "modal"; count: ModalCount; options: Sentence[][] }
    | { effect: "copy-spell"; what: ObjectRef }
    | { effect: "choose-new-targets" }
    | { effect: "regenerate"; what: ObjectRef }
    | { effect: "prevent-combat-damage"; by?: ObjectRef }
    | { effect: "become"; what: ObjectRef; spec: BecomeSpec }
    | { effect: "cant"; what: ObjectRef; action: CantAction }
  );

export interface LookTop { top: Amount; of: ZoneRef }

export type ModalCount =
  | { exactly: number }
  | { min: number; max: number }
  | { atLeast: number }; // "one or more"

export type DamageTarget =
  | { to: "any-target" }
  | { to: "object"; ref: ObjectRef }
  | { to: "player"; ref: PlayerRef }
  | { to: "each"; refs: (ObjectRef | PlayerRef)[] }
  | { to: "divided"; among: ObjectRef };

export type GainedAbility =
  | { gained: "keyword"; keyword: KeywordInstance }
  | { gained: "quoted"; ability: Ability };

export interface TokenSpec {
  name?: string;
  power?: Amount;
  toughness?: Amount;
  colors: Color[];
  supertypes: string[];
  types: string[];
  subtypes: string[];
  keywords: KeywordInstance[];
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
  inAddition?: boolean;
}

export type CantAction = "attack" | "block" | "attack-or-block" | "be-blocked" | "untap" | "be-countered";

export type ManaProduction =
  | { mana: "fixed"; symbols: string[] }                    // Add {G}{G}
  | { mana: "choice"; options: string[][] }                 // Add {R} or {G}
  | { mana: "any-color"; amount: Amount }                   // Add two mana of any color
  | { mana: "any-one-color"; amount: Amount }               // Add three mana of any one color
  | { mana: "commander-color"; amount: Amount };

export interface CounterSpec {
  /** "+1/+1", "-1/-1", or a named counter: "charge", "loyalty", "stun" … */
  type: string;
}

// ---------------------------------------------------------------------------
// Static effects
// ---------------------------------------------------------------------------

export type StaticEffect =
  | {
      static: "modify";
      what: ObjectRef;
      modifiers: StaticModifier[];
      condition?: Condition;
      duration?: Duration;
    }
  | { static: "cda-pt"; what: ObjectRef; stat: "power" | "toughness" | "both"; value: Amount }
  | { static: "enters-tapped"; what: ObjectRef }
  | { static: "enters-with-counters"; what: ObjectRef; counter: CounterSpec; count: Amount }
  | { static: "cant"; what: ObjectRef; action: CantAction; condition?: Condition }
  | { static: "must-attack"; what: ObjectRef };

export type StaticModifier =
  | { modifier: "pt"; power: SignedAmount; toughness: SignedAmount }
  | { modifier: "abilities"; abilities: GainedAbility[] };

// ---------------------------------------------------------------------------
// Conditions & durations
// ---------------------------------------------------------------------------

export type Condition =
  | { condition: "control"; who: PlayerRef; what: ObjectRef }
  | { condition: "you-do" }
  | { condition: "you-dont" }
  | { condition: "pays"; who: PlayerRef; cost: Cost[] }
  | { condition: "status"; what: ObjectRef; status: string }
  | { condition: "is-filter"; what: ObjectRef; filter: ObjectFilter }
  | { condition: "life-total"; who: PlayerRef; comparison: Comparison }
  | { condition: "your-turn" }
  | { condition: "not-your-turn" }
  | { condition: "remains"; what: ObjectRef; zone: ZoneRef }
  | { condition: "cards-in-hand"; who: PlayerRef; comparison: Comparison }
  | { condition: "raw"; text: string };

export type Duration =
  | { duration: "end-of-turn" }
  | { duration: "this-turn" }
  | { duration: "as-long-as"; condition: Condition }
  | { duration: "your-next-turn" };

// ---------------------------------------------------------------------------
// References: players, objects, filters
// ---------------------------------------------------------------------------

export type PlayerRef =
  | { player: "you" }
  | { player: "each-player" }
  | { player: "each-opponent" }
  | { player: "target-player" }
  | { player: "target-opponent" }
  | { player: "an-opponent" }
  | { player: "that-player" }
  | { player: "controller"; of: ObjectRef }
  | { player: "owner"; of: ObjectRef }
  | { player: "defending-player" };

export type ObjectRef =
  | { ref: "self" }                                          // ~
  | { ref: "it" }
  | { ref: "them" }
  | { ref: "this"; noun: string }                            // "this creature", "this spell"
  | { ref: "that"; filter: ObjectFilter }                    // "that card", "those creatures"
  | { ref: "target"; filter: ObjectFilter; count: Amount; upTo?: boolean }
  | { ref: "any-target" }
  | { ref: "each"; filter: ObjectFilter }
  | { ref: "all"; filter: ObjectFilter }
  | { ref: "filter"; filter: ObjectFilter; count: Amount; upTo?: boolean } // "a creature", "up to seven lands"
  | { ref: "equipped" | "enchanted"; noun: string };         // "equipped creature"

/**
 * Structured card/object filter. Fields are conjunctive; arrays within a
 * field are disjunctive where noted.
 */
export interface ObjectFilter {
  /** Card types, or-joined: "artifact or enchantment". */
  types?: string[];
  /** "noncreature", "nonland" … */
  nonTypes?: string[];
  /** Object class this filter selects over. */
  cls?: "permanent" | "card" | "spell" | "token" | "ability";
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
  /** "another"/"other" — not the source object. */
  other?: boolean;
  control?: "you" | "opponent" | "not-you" | "that-player";
  own?: "you" | "opponent";
  zone?: ZoneRef;
  properties?: PropertyConstraint[];
  /** "that targets …" */
  targets?: ObjectRef;
  /** "…or player": the filter unions in players ("target creature or player"). */
  orPlayer?: boolean;
  /** Unstructured trailing qualifier we chose to keep (rare, explicit). */
  withAbility?: string[];                                    // "with flying"
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
  owner?: "your" | "their" | "its-owner" | "each-player" | "any" | "an-opponent" | "that-player";
};

export type Zone = "battlefield" | "graveyard" | "library" | "hand" | "exile" | "stack" | "command";

export type Color = "white" | "blue" | "black" | "red" | "green";

// ---------------------------------------------------------------------------
// Amounts
// ---------------------------------------------------------------------------

export type Amount =
  | { amount: "fixed"; value: number }
  | { amount: "x" }
  | { amount: "count"; of: ObjectFilter }                    // "the number of Elves you control"
  | { amount: "attribute"; of: ObjectRef; attribute: "power" | "toughness" | "mana-value" }
  | { amount: "life-total"; of: PlayerRef }
  | { amount: "twice"; of: Amount }
  | { amount: "half"; of: Amount; round: "up" | "down" }
  | { amount: "plus"; a: Amount; b: Amount }
  | { amount: "that-much" };

export interface SignedAmount {
  sign: 1 | -1;
  amount: Amount;
}
