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
