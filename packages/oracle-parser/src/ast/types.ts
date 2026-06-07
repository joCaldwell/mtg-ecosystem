// packages/oracle-parser/src/ast/types.ts
// Discriminated union types for the AST representation of MTG cards

export interface CardIR {
  card_id: string;
  name: string;
  mana_cost?: ManaCost;
  cmc: number;
  types: string[];
  subtypes?: string[];
  supertypes?: string[];
  power?: string;
  toughness?: string;
  loyalty?: string;
  abilities: Ability[];
}

export interface ManaCost {
  generic?: number;
  white?: number;
  blue?: number;
  black?: number;
  red?: number;
  green?: number;
  colorless?: number;
}

export type Ability =
  | KeywordAbility
  | ActivatedAbility
  | TriggeredAbility
  | StaticAbility
  | SpellEffect
  | AsAbility
  | QuotedAbility;

export interface KeywordAbility {
  abilityWord?: string;
  kind: "keyword";
  keyword: string;
  cost?: any; // e.g., Evoke cost
}

export interface ActivatedAbility {
  abilityWord?: string;
  kind: "activated";
  cost: any;
  effects: any[];
}

export interface TriggeredAbility {
  abilityWord?: string;
  kind: "triggered";
  trigger: any;
  effects: any[];
}

export interface StaticAbility {
  abilityWord?: string;
  kind: "static";
  effects: any[];
  conditions?: any[];
}

export interface SpellEffect {
  abilityWord?: string;
  kind: "spell_effect";
  effects: any[];
}

// Pillar 1: Value Expressions
export type ValueExpression =
  | NumberValue
  | VariableValue
  | AttributeValue
  | CardFilterCountValue
  | MathExpression;

export interface NumberValue {
  type: "number";
  value: number;
}

export interface VariableValue {
  type: "variable";
  name: string; // e.g., "X"
}

export interface AttributeValue {
  type: "attribute";
  attribute: "power" | "toughness";
  source: "its";
}

export interface CardFilterCountValue {
  type: "card_filter_count";
  filter: CardFilter;
  location?: LocationSpec;
}

export interface MathExpression {
  type: "math";
  operation: "plus" | "twice" | "half";
  operands: ValueExpression[];
  rounded?: "up" | "down";
}

// Pillar 2: Composable Card Filters
export interface CardFilter {
  qualifiers: string[]; // e.g., "tapped", "nongreen", "legendary", "other", "Elf"
  cardType: string; // e.g., "creature", "land", "artifact"
  isCard?: boolean; // true if followed by the word "card"
  location?: LocationSpec;
  targeting?: TargetingClause;
}

export interface TargetingClause {
  only?: boolean;
  count?: string | number;
  target?: any;
}

export interface LocationSpec {
  relation: "from" | "in" | "on_the_battlefield";
  zone?: string; // "battlefield", "graveyard", "library", "hand", "exile"
}

// Pillar 3 & 4: Choices, Conditionals, and Durations
export interface ChoiceAbility {
  kind: "choice";
  choicesCount: string; // e.g. "one", "two", "one or both"
  options: Ability[];
}

export interface ConditionalEffect {
  type: "conditional";
  condition: Condition;
  thenEffects: any[];
  otherwiseEffects?: any[];
}

export type Condition =
  | ControlCondition
  | PayCostCondition
  | IfYouDoCondition
  | StatusCondition
  | TurnCondition
  | RemainsCondition;

export interface ControlCondition {
  type: "control";
  controller: "you" | "your";
  target: any;
}

export interface PayCostCondition {
  type: "pay_cost";
  target: any;
  cost: any;
}

export interface IfYouDoCondition {
  type: "if_you_do";
}

export interface StatusCondition {
  type: "status";
  target: any;
  status: "tapped" | "untapped" | "attacking" | "blocking";
}

export interface TurnCondition {
  type: "turn";
  turnOwner: "you" | "your" | "opponent";
}

export interface RemainsCondition {
  type: "remains";
  target: "it";
  location: LocationSpec;
}

export type Duration =
  | { type: "until_end_of_turn" }
  | { type: "this_turn" }
  | { type: "for_as_long_as"; condition: Condition }
  | { type: "during_turn"; turnOwner: "you" | "your" | "player" };

export interface ExileEffect {
  type: "exile";
  target: any;
  location?: LocationSpec;
}

export interface GainLifeEffect {
  type: "gain_life";
  target: any;
  life: ValueExpression;
}

export interface GainKeywordEffect {
  type: "gain_keyword";
  target: any;
  keywords: string[];
  duration?: Duration;
}

export interface BecomeEffect {
  type: "become";
  target: any;
  isCopy: boolean;
  copyTarget?: any;
  becomeFilter?: CardFilter;
  basePower?: number;
  baseToughness?: number;
  duration?: Duration;
  inAddition?: {
    types?: string[];
    colors?: string[];
  };
}

export interface ExileCost {
  type: "exile";
  target: any;
  fromZone?: string;
}

export interface PropertyQualifier {
  type: "property";
  property: "power" | "toughness" | "mana_value";
  value: ValueExpression;
  comparison?: "greater" | "less" | "equal" | "greater_or_equal" | "less_or_equal";
}

export interface CDAEffect {
  type: "cda";
  target: any;
  powerOnly: boolean;
  value: ValueExpression;
}

export interface GainControlEffect {
  type: "gain_control";
  target: any;
  duration?: Duration;
}

export interface PronounSelector {
  type: "pronoun";
  pronoun: "it" | "they" | "them" | "itself" | "themselves";
}

export interface RelativeSelector {
  type: "relative";
  determiner: "that" | "those";
  filter: CardFilter;
}

export interface TapEffect {
  type: "tap";
  target: any;
}

export interface UntapEffect {
  type: "untap";
  target: any;
}

export interface PutZoneEffect {
  type: "put_zone";
  target: any;
  location: LocationSpec;
  tapped?: boolean;
}

export interface PutCounterEffect {
  type: "put_counter";
  target: any;
  counterType: string;
  count: ValueExpression | string;
}

export interface SearchEffect {
  type: "search";
  zone: string;
  filter: CardFilter;
  actions?: any[];
  shuffle?: boolean;
}

export interface ShuffleEffect {
  type: "shuffle";
  zone?: string;
}

export interface ReturnCost {
  type: "return";
  target: any;
}

export interface AttackTrigger {
  type: "attack";
  actor: any;
  target?: any;
}

export interface BlockTrigger {
  type: "block";
  actor: any;
  becomesBlocked: boolean;
}

export interface PhaseTrigger {
  type: "phase_beginning";
  phase: string;
  turnOwner: "you" | "each_player" | "each_opponent" | "each" | "current_player";
  modifier?: string;
}

export interface AddManaEffect {
  type: "add_mana";
  mana: string[];
  anyColor?: boolean;
  amount?: ValueExpression;
}

export interface ChooseEffect {
  type: "choose";
  choice: string;
  filter?: CardFilter;
}

export interface QuotedAbility {
  kind: "quoted";
  ability: Ability;
}

export interface AsAbility {
  kind: "as";
  header: any;
  effects: any[];
}

export interface RevealEffect {
  type: "reveal";
  filter: CardFilter;
  location?: LocationSpec;
}

export interface ProtectionKeyword {
  type: "protection";
  from: any;
}

export interface HexproofFromKeyword {
  type: "hexproof_from";
  from: any;
}

export interface EnchantKeyword {
  type: "enchant";
  filter: CardFilter;
}

export interface LoyaltyCost {
  type: "loyalty";
  amount: string | number;
}




