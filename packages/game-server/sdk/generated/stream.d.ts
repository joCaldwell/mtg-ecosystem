/* Generated from Rust mtg-server schema; do not edit. */

/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "DecimalU64Counter".
 */
export type DecimalU64Counter = string;
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "Publication".
 */
export type Publication =
  | {
      kind: "outcome";
      result: RulesResult;
    }
  | {
      kind: "view_replaced";
      observation: Observation;
    }
  | {
      kind: "lifecycle";
      status: ExecutionStatus;
    };
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "RulesResult".
 */
export type RulesResult =
  | {
      outcome: "win";
      player: number;
    }
  | {
      outcome: "draw";
    };
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "PlayerRequest".
 */
export type PlayerRequest =
  | {
      candidates: string[];
      player: number;
      request: "opening_placement";
      token: string;
    }
  | {
      object: string;
      /**
       * OptionalEffect(true) selects this alternative to exile. Omission selects the printed shuffle.
       */
      offered?: GraveyardOrderOption | null;
      player: number;
      request: "graveyard_replacement_order";
      token: string;
    }
  | {
      candidates: AdditionalCostChoice[];
      player: number;
      request: "additional_costs";
      token: string;
    }
  | {
      candidates: string[];
      player: number;
      request: "entry_copy";
      source: string;
      token: string;
    }
  | {
      candidates: string[];
      player: number;
      request: "entry_attachment";
      source: string;
      token: string;
    }
  | {
      candidates: string[];
      player: number;
      previous_choices: string[];
      request: "legend_rule";
      token: string;
    }
  | {
      player: number;
      request: "hybrid_cost";
      symbols: ColorHybridSymbol[];
      token: string;
    }
  | {
      player: number;
      request: "announce_x";
      token: string;
    }
  | {
      candidates: string[];
      player: number;
      request: "sacrifice_cost";
      token: string;
    }
  | {
      candidates: string[];
      player: number;
      previous_choices: [number, string][];
      request: "forced_sacrifice";
      token: string;
    }
  | {
      choices: ManaAbilityIdentity[];
      mana_ability_source: string;
      player: number;
      request: "choose_mana_ability";
      token: string;
    }
  | {
      colors: Color[];
      player: number;
      request: "mana_combination";
      token: string;
      total: ExactQuantity;
    }
  | {
      colors: Color[];
      mana_ability_source?: string | null;
      player: number;
      request: "mana_color";
      token: string;
    }
  | {
      candidates: string[];
      player: number;
      request: "entry_reveal";
      source: string;
      token: string;
    }
  | {
      player: number;
      request: "optional_effect";
      token: string;
    }
  | {
      bottom_if_mulligan: number;
      can_mulligan: boolean;
      mulligans_taken: number;
      player: number;
      request: "mulligan";
      token: string;
    }
  | {
      candidates: string[];
      count: number;
      player: number;
      request: "bottom_opening";
      token: string;
    }
  | {
      allow_repeat?: boolean;
      candidates: number[];
      exactly?: number | null;
      one_or_more?: boolean;
      player: number;
      request: "modes";
      token: string;
    }
  | {
      candidates: VisibleCounterPlacementCandidate[];
      player: number;
      recipient: string;
      request: "counter_placement_effect";
      token: string;
    }
  | {
      candidates: VisibleDamageEffectCandidate[];
      player: number;
      recipient: PlayerTargetEntity;
      request: "damage_effect";
      token: string;
    }
  | {
      candidates: VisiblePendingTrigger[];
      player: number;
      request: "order_triggers";
      token: string;
    }
  | {
      candidates: PlayerTargetEntity[];
      occurrences: TargetSpec[];
      player: number;
      request: "entity_targets";
      token: string;
    }
  | {
      candidates: string[];
      occurrences: TargetSpec[];
      player: number;
      request: "stack_targets";
      token: string;
    }
  | {
      candidates: number[];
      occurrences: TargetSpec[];
      player: number;
      request: "player_targets";
      token: string;
    }
  | {
      candidates: string[];
      player: number;
      request: "sacrifice";
      token: string;
    }
  | {
      candidates: string[];
      count: number;
      player: number;
      request: "resolve_discard";
      token: string;
    }
  | {
      available: {
        B?: ExactQuantity;
        C?: ExactQuantity;
        G?: ExactQuantity;
        R?: ExactQuantity;
        U?: ExactQuantity;
        W?: ExactQuantity;
      };
      cost: ManaCost;
      /**
       * The permanent whose mana ability this nested payment or color choice
       * belongs to (CR 605.3a). Omitted outside nested mana-ability requests.
       */
      mana_ability_source?: string | null;
      player: number;
      request: "spend_mana";
      restricted_available?: RestrictedMana[];
      token: string;
    }
  | {
      designation: string;
      destination: Zone;
      object: string | null;
      player: number;
      previous_choices?: VisibleCommanderReturnChoice[];
      request: "commander_return";
      token: string;
    }
  | {
      candidates: string[];
      defenders: number[];
      player: number;
      request: "attackers";
      token: string;
    }
  | {
      attackers: string[];
      candidates: string[];
      legal_attackers?: LegalBlockers[];
      menace_attackers?: string[];
      player: number;
      request: "blockers";
      token: string;
    }
  | {
      lethal_requirements?: [string, ExactQuantity][];
      player: number;
      player_recipient?: number | null;
      power: ExactQuantity;
      recipients: string[];
      request: "damage";
      source: string;
      token: string;
    }
  | {
      candidates: string[];
      count: number;
      player: number;
      request: "discard";
      token: string;
    }
  | {
      player: number;
      request: "priority";
      token: string;
    }
  | {
      candidates: string[];
      counter_division?: CounterDivision | null;
      occurrences: TargetSpec[];
      player: number;
      request: "targets";
      token: string;
    }
  | {
      cost: ManaCost;
      /**
       * The permanent whose mana ability this nested payment or color choice
       * belongs to (CR 605.3a). Omitted outside nested mana-ability requests.
       */
      mana_ability_source?: string | null;
      player: number;
      request: "mana";
      token: string;
    };
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "GraveyardOrderOption".
 */
export type GraveyardOrderOption = "regenerate";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "Color".
 */
export type Color = "W" | "U" | "B" | "R" | "G" | "C";
/**
 * This interface was referenced by `undefined`'s JSON-Schema definition
 * via the `patternProperty` "^\d+$".
 *
 * This interface was referenced by `undefined`'s JSON-Schema definition
 * via the `patternProperty` "^\d+$".
 *
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "ExactQuantity".
 */
export type ExactQuantity = string;
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "HybridPayment".
 */
export type HybridPayment = "pay_two_life" | Color;
/**
 * The intrinsic ability for a basic land type or an index into this source's printed mana abilities.
 *
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "ManaAbilityIdentity".
 */
export type ManaAbilityIdentity =
  | {
      ability: "granted_land_tap";
      color: Color;
    }
  | {
      ability: "intrinsic";
      color: Color;
    }
  | {
      ability: "printed";
      index: number;
    };
/**
 * CR 614.1a/614.5: positive placement quantities only; removal is a cost.
 *
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "CounterPlacementModification".
 */
export type CounterPlacementModification =
  | {
      amount: ExactQuantity;
      operation: "add";
    }
  | {
      factor: ExactQuantity;
      operation: "multiply";
    };
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "DamageEffectKind".
 */
export type DamageEffectKind =
  | "double_damage"
  | "prevent_combat_damage"
  | "protection_from_each_color"
  | "protection_from_everything"
  | "protection_from_color";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "PlayerTargetEntity".
 */
export type PlayerTargetEntity =
  | {
      entity: "player";
      player: number;
    }
  | {
      entity: "object";
      object: string;
    };
/**
 * Public origin uses only already captured public/historical handles.
 *
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleTriggerAbility".
 */
export type VisibleTriggerAbility =
  | {
      chapter: number;
      origin: "delayed_chapter";
    }
  | {
      index: number;
      origin: "intrinsic";
    }
  | {
      oracle_id: string;
      origin: "granted";
      source: VisibleTarget;
      static_ability_index: number;
    }
  | {
      effect?: string | null;
      oracle_id: string;
      origin: "temporary_grant";
      source: VisibleTarget;
      triggered_ability_index: number;
    };
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "Chooser".
 */
export type Chooser = "controller";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "TargetDomain".
 */
export type TargetDomain =
  | (
      | "creature_or_planeswalker"
      | "battlefield_permanent"
      | "controlled_battlefield_permanent"
      | "controlled_battlefield_creature"
      | "battlefield_creature"
      | "own_graveyard_creature_card"
      | "own_graveyard_card"
      | "opponent_player"
      | "stack_spell"
      | "controlled_creature_spell"
      | "stack_activated_or_triggered_ability"
      | "player_or_planeswalker"
      | "any_player"
    )
  | {
      player_graveyard_cards: {
        maximum: number;
        owner_slot: number;
      };
    }
  | "blocking_battlefield_creature"
  | "any_damage_target";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "CardKind".
 */
export type CardKind = "land" | "creature" | "instant" | "sorcery" | "enchantment" | "artifact";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "CreaturePowerFilter".
 */
export type CreaturePowerFilter =
  | {
      comparison: "at_least";
      value: ExactQuantity;
    }
  | {
      comparison: "at_most";
      value: ExactQuantity;
    };
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "EvaluationStage".
 */
export type EvaluationStage = "announcement";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "ManaSpendRestriction".
 */
export type ManaSpendRestriction =
  | ("activated_ability" | "colorless_spell_or_activated_ability" | "creature_spell_or_battlefield_creature_ability")
  | {
      spell_subtypes: {
        /**
         * @minItems 3
         * @maxItems 3
         */
        any_of: [ManaSpellSubtype, ManaSpellSubtype, ManaSpellSubtype];
      };
    };
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "ManaSpellSubtype".
 */
export type ManaSpellSubtype = "dwarf" | "equipment" | "saga";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "Zone".
 */
export type Zone =
  | {
      zone: "battlefield";
    }
  | {
      owner: number;
      zone: "hand";
    }
  | {
      owner: number;
      zone: "graveyard";
    }
  | {
      owner: number;
      zone: "library";
    }
  | {
      owner: number;
      zone: "command";
    }
  | {
      zone: "exile";
    }
  | {
      zone: "stack";
    };
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "DecisionSourceKind".
 */
export type DecisionSourceKind = "spell" | "activated" | "triggered" | "mana_ability" | "effect";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleEffect".
 */
export type VisibleEffect =
  | {
      effect: "regeneration_shield";
      expires_in_cleanup_of_turn: number;
      handle: string;
      origin: VisibleEffectSource;
    }
  | {
      chapter: number;
      effect: "delayed_chapter";
      expires_in_cleanup_of_turn: number;
      origin: VisibleEffectSource;
    }
  | {
      effect: "protect_controller_until_next_turn";
      handle: string;
      origin: VisibleEffectSource;
    }
  | {
      effect: "remove_indestructible_set";
      expires_in_cleanup_of_turn: number;
      handle: string;
      origin: VisibleEffectSource;
      targets: VisibleTarget[];
    }
  | {
      effect: "grant_keywords_set";
      expires_in_cleanup_of_turn: number;
      handle: string;
      keywords: KeywordAbility[];
      modifier?: CreatureStatModifier | null;
      origin: VisibleEffectSource;
      targets: VisibleTarget[];
    }
  | {
      effect: "pump_set";
      expires_in_cleanup_of_turn: number;
      handle: string;
      origin: VisibleEffectSource;
      power: ExactQuantity;
      targets: VisibleTarget[];
      toughness: ExactQuantity;
    }
  | {
      effect: "exile_damaged";
      expires_in_cleanup_of_turn: number;
      handle: string;
      origin: VisibleEffectSource;
      targets: VisibleTarget[];
    }
  | {
      effect: "grant_keywords";
      expires_in_cleanup_of_turn: number;
      handle: string;
      keywords: KeywordAbility[];
      modifier?: CreatureStatModifier | null;
      origin: VisibleEffectSource;
      target: VisibleTarget;
    }
  | {
      effect: "modify_power_toughness";
      expires_in_cleanup_of_turn: number;
      handle: string;
      origin: VisibleEffectSource;
      power: ExactQuantity;
      target: VisibleTarget;
      toughness: ExactQuantity;
    }
  | {
      effect: "prevent_combat_damage";
      expires_in_cleanup_of_turn: number;
      handle: string;
      origin: VisibleEffectSource;
    }
  | {
      count: ExactQuantity;
      effect: "additional_land_plays";
      expires_in_cleanup_of_turn: number;
      handle: string;
      origin: VisibleEffectSource;
    }
  | {
      effect: "double_damage_to_permanent_or_player";
      handle: string;
      origin: VisibleEffectSource;
    }
  | {
      effect: "double_controller_creature_damage_to_permanent_or_player";
      handle: string;
      origin: VisibleEffectSource;
    };
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "KeywordAbility".
 */
export type KeywordAbility =
  | "flash"
  | "hexproof"
  | "shroud"
  | "defender"
  | "flying"
  | "reach"
  | "vigilance"
  | "haste"
  | "lifelink"
  | "deathtouch"
  | "indestructible"
  | "trample"
  | "first strike"
  | "double strike"
  | "menace"
  | "plainswalk"
  | "islandwalk"
  | "swampwalk"
  | "mountainwalk"
  | "forestwalk"
  | "protection_from_white"
  | "protection_from_blue"
  | "protection_from_black"
  | "protection_from_red"
  | "protection_from_green"
  | "protection_from_each_color"
  | "protection_from_everything";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "CreatureRestriction".
 */
export type CreatureRestriction = "cant_block" | "cant_be_blocked" | "cant_attack_or_block" | "can_block_only_flying";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "PlayerStatus".
 */
export type PlayerStatus = "live" | "left";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleSetupPhase".
 */
export type VisibleSetupPhase = "opening_actions" | "declare" | "bottom";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleEntityTarget".
 */
export type VisibleEntityTarget =
  | {
      entity: "player";
      player: number;
    }
  | {
      entity: "object";
      object: VisibleTarget;
    };
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "TurnStep".
 */
export type TurnStep =
  | "untap"
  | "upkeep"
  | "draw"
  | "precombat_main"
  | "beginning_combat"
  | "declare_attackers"
  | "declare_blockers"
  | "first_strike_damage"
  | "combat_damage"
  | "end_combat"
  | "postcombat_main"
  | "end"
  | "cleanup";
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "ExecutionStatus".
 */
export type ExecutionStatus =
  | {
      status: "queued";
    }
  | {
      status: "running";
    }
  | {
      status: "waiting";
    }
  | {
      status: "applied";
    }
  | {
      status: "cancelled";
    }
  | {
      status: "rejected";
    }
  | {
      status: "resource_paused";
    }
  | {
      status: "faulted";
    }
  | {
      status: "finished";
    };

export interface StreamPage {
  entries: StreamEntry[];
  has_more: boolean;
  next: DecimalU64Counter;
  protocol_version: number;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "StreamEntry".
 */
export interface StreamEntry {
  cursor: DecimalU64Counter;
  publication: Publication;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "Observation".
 */
export interface Observation {
  decision: PlayerRequest | null;
  decision_context?: DecisionContext | null;
  observation_version: number;
  view: PermittedView;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "AdditionalCostChoice".
 */
export interface AdditionalCostChoice {
  id: string;
  mana: ManaCost;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "ManaCost".
 */
export interface ManaCost {
  colored: Color[];
  generic: ExactQuantity;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "ColorHybridSymbol".
 */
export interface ColorHybridSymbol {
  /**
   * @minItems 2
   * @maxItems 2
   */
  colors: [HybridPayment, HybridPayment];
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleCounterPlacementCandidate".
 */
export interface VisibleCounterPlacementCandidate {
  amount_after: ExactQuantity;
  amount_before: ExactQuantity;
  effect: CounterPlacementEffectHandle;
  modification: CounterPlacementModification;
}
/**
 * A viewer's public provider reference plus the definition's ability index.
 * Decision tokens bind it to one exact instruction and applied prefix.
 *
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "CounterPlacementEffectHandle".
 */
export interface CounterPlacementEffectHandle {
  ability: number;
  source: string;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleDamageEffectCandidate".
 */
export interface VisibleDamageEffectCandidate {
  amount_after: ExactQuantity;
  amount_before: ExactQuantity;
  damage: string;
  damage_oracle_id: string;
  damage_source: VisibleTarget;
  effect: string;
  effect_controller: number;
  effect_oracle_id: string;
  effect_source: VisibleTarget;
  kind: DamageEffectKind;
  label?: string | null;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleTarget".
 */
export interface VisibleTarget {
  announced: string;
  current: string | null;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisiblePendingTrigger".
 */
export interface VisiblePendingTrigger {
  ability: VisibleTriggerAbility;
  handle: string;
  label?: string | null;
  oracle_id: string;
  source: VisibleTarget;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "TargetSpec".
 */
export interface TargetSpec {
  chooser: Chooser;
  domain: TargetDomain;
  filter?: PermanentFilter | null;
  selection_id: string;
  slot: number;
  stage: EvaluationStage;
}
/**
 * Finite CR 115 filters; an empty type set means any permanent. Types are
 * alternatives; exclusions, controller and tapped restrictions are intersections.
 *
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "PermanentFilter".
 */
export interface PermanentFilter {
  non_colors: Color[];
  non_types: CardKind[];
  opponent: boolean;
  /**
   * Derived creature power at target selection and resolution (CR 601.2c, 608.2b).
   */
  power?: CreaturePowerFilter;
  /**
   * Require the current battlefield incarnation to be tapped (CR 110.5,
   * 601.2c, 608.2b). False imposes no tap-status restriction.
   */
  tapped?: boolean;
  types: CardKind[];
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "RestrictedMana".
 */
export interface RestrictedMana {
  amount: ExactQuantity;
  color: Color;
  restriction: ManaSpendRestriction;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleCommanderReturnChoice".
 */
export interface VisibleCommanderReturnChoice {
  designation: string;
  owner: number;
  to_command: boolean;
}
/**
 * Public declaration hints, scoped to current attacker/blocker handles.
 *
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "LegalBlockers".
 */
export interface LegalBlockers {
  attackers: string[];
  blocker: string;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "CounterDivision".
 */
export interface CounterDivision {
  count: ExactQuantity;
  maximum_targets: number;
  minimum_targets: number;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "DecisionContext".
 */
export interface DecisionContext {
  instruction?: DecisionInstruction | null;
  modes?: DecisionMode[];
  source?: DecisionSource | null;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "DecisionInstruction".
 */
export interface DecisionInstruction {
  source_block: number;
  text: string;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "DecisionMode".
 */
export interface DecisionMode {
  mode: number;
  text: string;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "DecisionSource".
 */
export interface DecisionSource {
  ability?: number | null;
  kind: DecisionSourceKind;
  object?: string | null;
  oracle_id: string;
  text: string;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "PermittedView".
 */
export interface PermittedView {
  active: number | null;
  combat: VisibleAttacker[];
  commanders: VisibleCommander[];
  effects?: VisibleEffect[];
  known_library_positions?: KnownLibraryPosition[];
  known_searched_hand?: [number, string][];
  live_turn_order: number[];
  objects: VisibleObject[];
  players: VisiblePlayer[];
  priority: number | null;
  sacrifice_choices?: [number, string][];
  setup?: VisibleSetup | null;
  stack: VisibleStackEntry[];
  turn: TurnState;
  turn_owner: number;
  viewer: number;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleAttacker".
 */
export interface VisibleAttacker {
  attacker: string;
  blocked: boolean;
  blockers: string[];
  defender: number;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleCommander".
 */
export interface VisibleCommander {
  designation: string;
  history: CommanderHistory;
  oracle_id: string;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "CommanderHistory".
 */
export interface CommanderHistory {
  color_identity: Color[];
  combat_damage: {
    [k: string]: ExactQuantity;
  };
  command_casts: {
    [k: string]: ExactQuantity;
  };
  owner: number;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleEffectSource".
 */
export interface VisibleEffectSource {
  controller: number;
  oracle_id: string;
  source: VisibleTarget;
}
/**
 * Fixed signed power/toughness changes belonging to one complete creature effect.
 *
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "CreatureStatModifier".
 */
export interface CreatureStatModifier {
  power: ExactQuantity;
  toughness: ExactQuantity;
}
/**
 * A remembered public tuck. Position is one-based; memory gives no object
 * handle or authority to inspect or target a card in a hidden library.
 *
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "KnownLibraryPosition".
 */
export interface KnownLibraryPosition {
  library_owner: number;
  oracle_id: string;
  position: number;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleObject".
 */
export interface VisibleObject {
  attached_to?: string | null;
  attachments?: string[];
  attack_ready: boolean;
  characteristics?: Characteristics | null;
  controller: number;
  copied_values?: VisiblePermanentCopy | null;
  display_face?: string | null;
  entered_this_turn?: boolean;
  exiled_by?: string | null;
  face_down?: boolean;
  handle: string;
  keywords?: KeywordAbility[];
  lore_counters?: ExactQuantity;
  mana_ability_used?: boolean | null;
  marked_damage: ExactQuantity;
  oracle_id: string;
  owner: number;
  phased_out?: boolean;
  plus_one_counters?: ExactQuantity;
  restrictions?: CreatureRestriction[];
  tapped: boolean;
  token?: VisibleToken | null;
  zone: Zone;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "Characteristics".
 */
export interface Characteristics {
  /**
   * Other simultaneous card types; empty preserves existing catalog bytes.
   */
  additional_types?: CardKind[];
  basic: boolean;
  color_identity: Color[];
  colors: Color[];
  hybrid_mana?: ColorHybridSymbol[];
  /**
   * Canonical execution type: creature first, then land, artifact, enchantment.
   */
  kind: CardKind;
  legendary: boolean;
  mana_cost: ManaCost;
  power: ExactQuantity | null;
  /**
   * CR 205.4g; omitted for nonsnow characteristics.
   */
  snow?: boolean;
  subtypes: string[];
  toughness: ExactQuantity | null;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisiblePermanentCopy".
 */
export interface VisiblePermanentCopy {
  name: string;
  oracle_id?: string | null;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleToken".
 */
export interface VisibleToken {
  keywords: KeywordAbility[];
  name: string;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisiblePlayer".
 */
export interface VisiblePlayer {
  /**
   * Public viewer handles, grouped by physical owner (CR406.1–3).
   */
  exile?: string[];
  hand_count: number;
  library_count: number;
  library_shuffles?: number;
  life: ExactQuantity;
  life_frozen_until_next_turn?: boolean;
  mana: {
    B?: ExactQuantity;
    C?: ExactQuantity;
    G?: ExactQuantity;
    R?: ExactQuantity;
    U?: ExactQuantity;
    W?: ExactQuantity;
  };
  player: number;
  restricted_mana?: RestrictedMana[];
  status: PlayerStatus;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleSetup".
 */
export interface VisibleSetup {
  phase: VisibleSetupPhase;
  players: VisibleSetupPlayer[];
  round: number;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleSetupPlayer".
 */
export interface VisibleSetupPlayer {
  kept: boolean;
  /**
   * Accepted declaration rule-steps, including the current round before redraw.
   */
  mulligans_taken: number;
  player: number;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleStackEntry".
 */
export interface VisibleStackEntry {
  ability?: VisibleAbility | null;
  bestowed?: boolean;
  cast_complete: boolean;
  controller: number;
  entity_targets?: VisibleEntityTarget[];
  handle?: string | null;
  hybrid_colors?: HybridPayment[];
  mana_value?: ExactQuantity | null;
  mode?: number | null;
  player_targets?: number[];
  selected_additional_costs?: string[] | null;
  selected_modes?: number[];
  spell: string | null;
  spell_copy?: boolean;
  stack_targets?: VisibleStackTarget[];
  targets: VisibleTarget[];
  triggered?: VisibleTriggered | null;
  x?: ExactQuantity | null;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleAbility".
 */
export interface VisibleAbility {
  ability: number;
  handle: string;
  oracle_id: string;
  source: VisibleTarget;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleStackTarget".
 */
export interface VisibleStackTarget {
  announced: string;
  current: string | null;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "VisibleTriggered".
 */
export interface VisibleTriggered {
  ability: VisibleTriggerAbility;
  handle: string;
  oracle_id: string;
  source: VisibleTarget;
}
/**
 * This interface was referenced by `StreamPage`'s JSON-Schema
 * via the `definition` "TurnState".
 */
export interface TurnState {
  lands_played: number;
  number: number;
  step: TurnStep;
}
