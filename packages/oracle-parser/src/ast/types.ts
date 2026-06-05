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
  | SpellEffect;

export interface KeywordAbility {
  kind: "keyword";
  keyword: string;
  cost?: any; // e.g., Evoke cost
}

export interface ActivatedAbility {
  kind: "activated";
  cost: any;
  effects: any[];
}

export interface TriggeredAbility {
  kind: "triggered";
  trigger: any;
  effects: any[];
}

export interface StaticAbility {
  kind: "static";
  effects: any[];
  conditions?: any[];
}

export interface SpellEffect {
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


