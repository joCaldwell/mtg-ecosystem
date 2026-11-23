/* Generated from Rust mtg-server schema; do not edit. */

/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ActivationCost".
 */
export type ActivationCost =
  | ("sacrifice_source" | "tap_source")
  | {
      sacrifice_permanent: {
        filter: SacrificeFilter;
        mana?: ManaCost | null;
      };
    }
  | {
      mana: {
        cost: ManaCost;
      };
    }
  | {
      composite: {
        costs: PermanentActivationCosts;
      };
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CardKind".
 */
export type CardKind = "land" | "creature" | "instant" | "sorcery" | "enchantment" | "artifact";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Color".
 */
export type Color = "W" | "U" | "B" | "R" | "G" | "C";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ExactQuantity".
 */
export type ExactQuantity = string;
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "FunctioningZone".
 */
export type FunctioningZone = "battlefield" | "stack";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Instruction".
 */
export type Instruction =
  | {
      action: SetMove;
      op: "move_set";
      set: ObjectSet;
      source_block: number;
    }
  | {
      op: "pump_set";
      power: ExactQuantity;
      set: ObjectSet;
      source_block: number;
      toughness: ExactQuantity;
    }
  | {
      amount: WipeAmount;
      each_player: boolean;
      op: "damage_set";
      sets: ObjectSet[];
      source_block: number;
    }
  | {
      op: "gain_life_destroyed";
      source_block: number;
    }
  | {
      op: "exile_damaged_this_turn";
      source_block: number;
    }
  | {
      op: "each_player_sacrifice";
      source_block: number;
    }
  | {
      amount: ExactQuantity;
      op: "lose_life_death_controller";
      source_block: number;
    }
  | {
      amount: SacrificeDamageAmount;
      op: "sacrifice_damage";
      optional: boolean;
      recipient: LifeRecipient | null;
      source_block: number;
      target_slot: number | null;
    }
  | {
      op: "each_opponent_sacrifice";
      source_block: number;
    }
  | {
      duration: Duration;
      op: "modify_source_power_toughness";
      power: ExactQuantity;
      source_block: number;
      toughness: ExactQuantity;
    }
  | {
      op: "destroy_permanent";
      optional?: boolean;
      regeneration_forbidden?: boolean;
      source_block: number;
      target_slot: number;
    }
  | {
      op: "exile_permanent";
      source_block: number;
      target_slot: number;
    }
  | {
      op: "untap_permanent";
      source_block: number;
      target_slot: number;
    }
  | {
      op: "tap_permanent";
      source_block: number;
      target_slot: number;
    }
  | {
      amount: TargetAmount;
      gain: boolean;
      op: "change_life_from_target";
      recipient: EffectRecipient;
      source_block: number;
    }
  | {
      count: number;
      op: "create_tokens_for";
      recipient: EffectRecipient;
      source_block: number;
      token: CreatureToken;
    }
  | {
      amount: ExactQuantity;
      op: "damage_target_controller";
      source_block: number;
      target_slot: number;
    }
  | {
      op: "attach_equipment";
      source_block: number;
      target_slot: number;
    }
  | {
      op: "return_aura_from_graveyard";
      source_block: number;
    }
  | {
      op: "destroy_target_creature";
      source_block: number;
      target_slot: number;
    }
  | {
      op: "destroy_all_creatures";
      source_block: number;
    }
  | {
      op: "return_own_graveyard_creature_to_hand";
      optional?: boolean;
      source_block: number;
      target_slot: number;
    }
  | {
      maximum: number;
      op: "search_basic_lands";
      source_block: number;
    }
  | {
      count: number;
      op: "create_creature_tokens";
      source_block: number;
      token: CreatureToken;
    }
  | {
      amount: ExactQuantity;
      on: CounterRecipient;
      op: "put_plus_one_counters";
      source_block: number;
    }
  | {
      duration: Duration;
      keywords: KeywordAbility[];
      modifier?: CreatureStatModifier | null;
      op: "grant_keywords_set";
      set: ObjectSet;
      source_block: number;
    }
  | {
      duration: Duration;
      keywords: KeywordAbility[];
      modifier?: CreatureStatModifier | null;
      op: "grant_keywords";
      source_block: number;
      target_slot: number;
    }
  | {
      duration: Duration;
      op: "modify_power_toughness";
      power: ExactQuantity;
      source_block: number;
      target_slot: number;
      toughness: ExactQuantity;
    }
  | {
      duration: Duration;
      op: "prevent_combat_damage";
      source_block: number;
    }
  | {
      amount: string;
      duration: Duration;
      op: "prevent_next_damage";
      source_block: number;
      target_slot: number;
    }
  | {
      op: "return_to_owner_hand";
      source_block: number;
      target_slot: number;
    }
  | {
      op: "put_on_top_owner_library";
      source_block: number;
      target_slot: number;
    }
  | {
      op: "target_player_sacrifice_creature";
      source_block: number;
      target_slot: number;
    }
  | {
      op: "sacrifice_creature";
      recipient: ActionRecipient;
      source_block: number;
    }
  | {
      count: ExactQuantity;
      op: "draw_cards";
      recipient?: LifeRecipient | null;
      source_block: number;
    }
  | {
      op: "optional_draw_cards";
      source_block: number;
    }
  | {
      op: "draw_damage_amount";
      source_block: number;
    }
  | {
      count: ExactQuantity;
      op: "discard_cards";
      recipient?: ActionRecipient | null;
      source_block: number;
    }
  | {
      op: "counter_stack_object";
      source_block: number;
      target_slot: number;
    }
  | {
      amount: string;
      op: "deal_damage";
      source_block: number;
      target_slot: number;
    }
  | {
      amount: string;
      op: "gain_life";
      recipient: LifeRecipient;
      source_block: number;
    }
  | {
      amount: string;
      op: "lose_life";
      recipient: LifeRecipient;
      source_block: number;
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SetMove".
 */
export type SetMove = "destroy" | "exile" | "hand";
/**
 * Closed non-targeted sets, evaluated as an instruction is performed (CR 608.2h).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ObjectSet".
 */
export type ObjectSet =
  | "controller_creatures"
  | "creatures"
  | "nonland_permanents"
  | "enchantments"
  | "attacking_creatures"
  | "opponent_creatures"
  | "flying_creatures"
  | "nonflying_creatures";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "WipeAmount".
 */
export type WipeAmount =
  | {
      amount: "fixed";
      value: ExactQuantity;
    }
  | {
      amount: "announced_x";
    }
  | {
      amount: "creature_count";
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SacrificeDamageAmount".
 */
export type SacrificeDamageAmount =
  | {
      amount: "fixed";
      value: ExactQuantity;
    }
  | {
      amount: "sacrificed_power";
    };
/**
 * Controller means the frozen invocation controller (CR 109.5), not the
 * current controller of its physical source. Target refers to AnyPlayer.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "LifeRecipient".
 */
export type LifeRecipient =
  | ("each_opponent" | "controller")
  | "event_player"
  | {
      target: {
        target_slot: number;
      };
    };
/**
 * Fixed-turn effects expire in the atomic cleanup action (CR 514.2).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Duration".
 */
export type Duration = "until_end_of_turn";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TargetAmount".
 */
export type TargetAmount =
  | {
      fixed: {
        amount: ExactQuantity;
      };
    }
  | {
      attribute: {
        attribute: TargetAttribute;
        target_slot: number;
      };
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TargetAttribute".
 */
export type TargetAttribute = "power" | "mana_value";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "EffectRecipient".
 */
export type EffectRecipient =
  | "controller"
  | {
      target_controller: {
        target_slot: number;
      };
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
  | "menace";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CounterRecipient".
 */
export type CounterRecipient =
  | {
      recipient: "source";
    }
  | {
      recipient: "target";
      target_slot: number;
    };
/**
 * Resolution actions use a separate binding domain from draw/life. The event
 * player is the frozen damaged player of a source-damage trigger (CR 603.2c).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ActionRecipient".
 */
export type ActionRecipient = "controller" | "event_player";
/**
 * Permanent resolution is distinct from an empty instruction program.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SpellResolution".
 */
export type SpellResolution = "instructions" | "permanent";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Chooser".
 */
export type Chooser = "controller";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TargetDomain".
 */
export type TargetDomain =
  | (
      | "battlefield_permanent"
      | "controlled_battlefield_creature"
      | "battlefield_creature"
      | "own_graveyard_creature_card"
      | "opponent_player"
      | "stack_spell"
      | "stack_activated_or_triggered_ability"
      | "any_player"
    )
  | "blocking_battlefield_creature"
  | "any_damage_target";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "EvaluationStage".
 */
export type EvaluationStage = "announcement";
/**
 * Supported announcement timing; permissions are checked by the runtime.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SpellTiming".
 */
export type SpellTiming = "instant" | "main_phase";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Capability".
 */
export type Capability =
  | "cast_trigger_v1"
  | "attack_trigger_v1"
  | "damage_trigger_v1"
  | "source_damage_trigger_v2"
  | "optional_controller_draw_v1"
  | "event_player_discard_v1"
  | "event_player_sacrifice_v1"
  | "filtered_damage_trigger_v1"
  | "damage_controller_discard_v1"
  | "damage_quantity_draw_v1"
  | "snow_supertype_v1"
  | "attacking_creature_keywords_v1"
  | "opponent_cast_prohibition_v1"
  | "grouped_creature_damage_trigger_v1"
  | "event_player_recipient_v1"
  | "source_status_condition_v1"
  | "step_beginning_trigger_v1"
  | "enters_tapped_v1"
  | "flash_v1"
  | "cast_artifact_v1"
  | "permanent_mana_v1"
  | "composite_activation_cost_v1"
  | "cast_enchantment_v1"
  | "aura_control_v1"
  | "temporary_power_toughness_v1"
  | "combat_damage_prevention_v1"
  | "damage_budget_prevention_v1"
  | "static_damage_replacement_v1"
  | "controlled_creature_damage_replacement_v1"
  | "normal_layout_v1"
  | "basic_land_mana_v1"
  | "land_play_v1"
  | "base_creature_v1"
  | "commander_identity_v1"
  | "cast_instant_v1"
  | "cast_creature_v1"
  | "creature_target_v1"
  | "blocking_creature_target_v1"
  | "tapped_permanent_target_v1"
  | "untap_permanent_v1"
  | "tap_permanent_v1"
  | "activated_tap_permanent_v1"
  | "tap_source_destroy_v1"
  | "tap_source_untap_creature_v1"
  | "activated_spell_counter_v1"
  | "activated_permanent_owner_hand_v1"
  | "owner_hand_move_v1"
  | "permanent_owner_hand_move_v1"
  | "cast_sorcery_v1"
  | "owner_library_top_move_v1"
  | "activated_ability_v1"
  | "sacrifice_source_cost_v1"
  | "controller_draw_v1"
  | "controller_discard_v1"
  | "opponent_target_v1"
  | "target_player_sacrifice_creature_v1"
  | "stack_spell_target_v1"
  | "stack_ability_target_v1"
  | "counter_stack_object_v1"
  | "tap_source_cost_v1"
  | "mana_activation_cost_v1"
  | "any_damage_target_v1"
  | "any_player_target_v1"
  | "noncombat_damage_v1"
  | "life_gain_v1"
  | "life_loss_v1"
  | "triggered_ability_v1"
  | "self_enters_battlefield_v1"
  | "other_creature_enters_battlefield_v1"
  | "creature_dies_v1"
  | "self_dies_v1"
  | "intervening_if_v1"
  | "modal_choice_v1"
  | "defender_v1"
  | "flying_v1"
  | "reach_v1"
  | "vigilance_v1"
  | "haste_v1"
  | "lifelink_v1"
  | "deathtouch_v1"
  | "indestructible_v1"
  | "temporary_indestructible_spell_v1"
  | "temporary_flying_spell_v1"
  | "temporary_deathtouch_spell_v1"
  | "compound_indestructible_spell_v1"
  | "temporary_creature_keyword_program_v1"
  | "temporary_creature_set_keyword_program_v1"
  | "trample_v1"
  | "first_strike_v1"
  | "double_strike_v1"
  | "menace_v1"
  | "creature_tokens_v1"
  | "plus_one_counters_v1"
  | "destroy_creature_v1"
  | "graveyard_recursion_v1"
  | "basic_land_search_v1"
  | "creature_anthem_v1"
  | "target_player_draw_v1"
  | "attachments_v1"
  | "permanent_removal_v1"
  | "sacrifice_v1"
  | "board_wipes_v1";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ManaProduction".
 */
export type ManaProduction =
  | {
      mana: "fixed";
      symbols: Color[];
    }
  | {
      mana: "any_color";
    }
  | {
      mana: "commander_color";
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "StaticEffect".
 */
export type StaticEffect =
  | (
      | "enters_tapped"
      | "reduce_generic_by_creature_count"
      | "cant_be_countered"
      | "double_damage_to_permanent_or_player"
    )
  | "control_enchanted_creature"
  | "opponents_cant_cast_during_controller_turn"
  | {
      modify_creatures: {
        keywords: KeywordAbility[];
        power: ExactQuantity;
        restrictions: CreatureRestriction[];
        scope: CreatureScope;
        toughness: ExactQuantity;
      };
    }
  | {
      controller_creature_anthem: {
        power: ExactQuantity;
        toughness: ExactQuantity;
      };
    }
  | "double_controller_creature_damage_to_permanent_or_player";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CreatureRestriction".
 */
export type CreatureRestriction = "cant_block" | "cant_be_blocked" | "cant_attack_or_block";
/**
 * Static scopes in the finite attachment pool (CR 611.3a, 613.1f/4c).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CreatureScope".
 */
export type CreatureScope = ("equipped" | "enchanted" | "controller" | "source") | "attacking_controller";
/**
 * Mandatory ordinary event patterns. ETB patterns inspect the event's
 * resulting battlefield, including the entering source itself (CR 603.6a).
 * Dies inspects the battlefield immediately before the actual graveyard move
 * (CR 603.10/700.4), including a source that dies simultaneously.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TriggerCondition".
 */
export type TriggerCondition =
  | ("opponent_creature_dies" | "self_enters_battlefield" | "self_dies" | "self_aura_to_graveyard")
  | {
      grouped_damage_dealt: {
        kind: TriggerDamageKind;
        recipient: DamageTriggerRecipient;
        source: DamageTriggerSource;
        source_power: DamageSourcePowerPredicate;
      };
    }
  | {
      damage_dealt: {
        kind: TriggerDamageKind;
        recipient: DamageTriggerRecipient;
        source: DamageTriggerSource;
      };
    }
  | {
      creature_attacked: {
        attacker: AttackSubject;
      };
    }
  | {
      spell_cast: {
        caster: CasterRelation;
        spell: TypePredicate;
      };
    }
  | {
      step_began: {
        step: BeginningStep;
        turn_relation: TurnRelation;
      };
    }
  | "other_creature_enters_battlefield"
  | "any_creature_dies";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TriggerDamageKind".
 */
export type TriggerDamageKind = "combat" | "any";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "DamageTriggerRecipient".
 */
export type DamageTriggerRecipient = "player" | "opponent_player";
/**
 * CR 120.2/120.7/603.2c: independent source, kind and recipient dimensions.
 * Source means the observer's exact incarnation; CreatureFilter separately
 * matches actual creature damage sources and requires FilteredDamageTriggerV1.
 * DamageTriggerV1 requires combat/player; V2 permits any kind or opponents.
 * These predicates alone supply no event-player or event-quantity binding.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "DamageTriggerSource".
 */
export type DamageTriggerSource =
  | {
      subject: "source";
    }
  | {
      controller: DamageSourceRelation;
      subject: "creature_filter";
      types: TypePredicate;
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "DamageSourceRelation".
 */
export type DamageSourceRelation = "controller";
/**
 * Source ObjectFilter fields are conjunctive; explicit anyOf retains OR.
 * Only full card-type membership is supported, never execution-kind equality.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TypePredicate".
 */
export type TypePredicate =
  | {
      predicate: "any";
    }
  | {
      kind: CardKind;
      predicate: "type";
    }
  | {
      kind: CardKind;
      predicate: "not_type";
    }
  | {
      predicate: "all";
      /**
       * @minItems 1
       * @maxItems 32
       */
      terms: [TypePredicate, ...TypePredicate[]];
    }
  | {
      predicate: "any_of";
      /**
       * @minItems 1
       * @maxItems 32
       */
      terms: [TypePredicate, ...TypePredicate[]];
    };
/**
 * Candidate-local event-time comparison; no printed-source pronoun binding.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "DamageSourcePowerPredicate".
 */
export type DamageSourcePowerPredicate = "greater_than_base";
/**
 * Ordinary per-creature declaration predicates (CR 508.3a/603.2c).
 * Controller relation and full type membership are independent of ownership.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "AttackSubject".
 */
export type AttackSubject =
  | {
      subject: "source";
    }
  | {
      controller: AttackerRelation;
      subject: "filter";
      types: TypePredicate;
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "AttackerRelation".
 */
export type AttackerRelation = "controller" | "any_player" | "opponent";
/**
 * CR 603.3a: caster relation is independent of the source's controller/owner.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CasterRelation".
 */
export type CasterRelation = "controller" | "any_player" | "opponent";
/**
 * Exact step predicates; phases, delayed and conditional boundaries are excluded.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "BeginningStep".
 */
export type BeginningStep = "upkeep" | "draw" | "end";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TurnRelation".
 */
export type TurnRelation = "controller" | "any_player" | "opponent";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "InterveningCondition".
 */
export type InterveningCondition =
  | "controller_hand_empty"
  | {
      source_status: {
        status: TapStatus;
      };
    };
/**
 * CR 110.5: only battlefield permanents have tapped/untapped status.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TapStatus".
 */
export type TapStatus = "tapped" | "untapped";

/**
 * Only public open-decklist facts. No seed, private profile digest or state ID.
 */
export interface PublicManifest {
  catalog: Catalog;
  catalog_sha256: string;
  /**
   * @minItems 4
   * @maxItems 4
   */
  decks: [Decklist, Decklist, Decklist, Decklist];
  engine_build_id: string;
  match_id: string;
  profile: string;
  protocol_version: number;
  server_build_id: string;
  starting_player: number;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Catalog".
 */
export interface Catalog {
  definitions: Definition[];
  rules_edition: string;
  rules_sha256: string;
  schema_version: 1;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Definition".
 */
export interface Definition {
  activated?: ActivatedAbility[];
  capabilities: Capability[];
  characteristics: Characteristics;
  intrinsic_zone: FunctioningZone | null;
  keywords?: KeywordAbility[];
  mana_abilities?: ManaAbility[];
  name: string;
  oracle_id: string;
  provenance: SourceProvenance;
  /**
   * Offline compiler-owned labels; absent on historical IR and synthetic definitions.
   */
  source_labels?: SourceLabels | null;
  spell: SpellProgram | null;
  static_abilities?: StaticAbility[];
  triggered?: TriggeredAbility[];
  x_mana_cost?: boolean;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ActivatedAbility".
 */
export interface ActivatedAbility {
  cost: ActivationCost;
  functioning_zone: FunctioningZone;
  program: SpellProgram;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SacrificeFilter".
 */
export interface SacrificeFilter {
  subtypes: string[];
  /**
   * A union of permanent types; empty means any permanent type.
   */
  types: CardKind[];
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ManaCost".
 */
export interface ManaCost {
  colored: Color[];
  generic: ExactQuantity;
}
/**
 * Finite permanent costs; costs are committed only after complete payment.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "PermanentActivationCosts".
 */
export interface PermanentActivationCosts {
  mana?: ManaCost | null;
  sacrifice: boolean;
  tap: boolean;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SpellProgram".
 */
export interface SpellProgram {
  additional_sacrifice?: SacrificeFilter | null;
  functioning_zone: FunctioningZone;
  instructions: Instruction[];
  modes?: SpellMode[];
  resolution: SpellResolution;
  targets: TargetSpec[];
  timing: SpellTiming;
}
/**
 * CR 111.3–4: precisely defined creature tokens, never copied card definitions.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CreatureToken".
 */
export interface CreatureToken {
  colors: Color[];
  keywords: KeywordAbility[];
  power: ExactQuantity;
  subtypes: string[];
  toughness: ExactQuantity;
}
/**
 * Fixed signed power/toughness changes belonging to one complete creature effect.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CreatureStatModifier".
 */
export interface CreatureStatModifier {
  power: ExactQuantity;
  toughness: ExactQuantity;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SpellMode".
 */
export interface SpellMode {
  program: SpellProgram;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "PermanentFilter".
 */
export interface PermanentFilter {
  non_colors: Color[];
  non_types: CardKind[];
  opponent: boolean;
  /**
   * Require the current battlefield incarnation to be tapped (CR 110.5,
   * 601.2c, 608.2b). False imposes no tap-status restriction.
   */
  tapped?: boolean;
  types: CardKind[];
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ManaAbility".
 */
export interface ManaAbility {
  costs: PermanentActivationCosts;
  production: ManaProduction;
  source_block: number;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SourceProvenance".
 */
export interface SourceProvenance {
  abilities_json: string;
  archive_manifest_sha256: string;
  ast_sha256: string;
  binding_sha256: string;
  parser_sha256: string;
  printing_id: string;
  source_index: number;
  /**
   * Exact compact archive subvalues, not executable text.
   */
  source_json: string;
  source_sha256: string;
}
/**
 * CR702.3: defender is a static prohibition on declaring this creature as
 * an attacker. Power and continuous-control eligibility do not override it.
 * Exact normalized source blocks and compiler index bindings, never executable text.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SourceLabels".
 */
export interface SourceLabels {
  activated: number[];
  blocks: string[];
  mana: number[];
  spell_modes: string[];
  statics: number[];
  text: string;
  triggered: number[];
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "StaticAbility".
 */
export interface StaticAbility {
  effect: StaticEffect;
  functioning_zone: FunctioningZone;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TriggeredAbility".
 */
export interface TriggeredAbility {
  condition: TriggerCondition;
  death_filter?: DeathFilter | null;
  functioning_zone: FunctioningZone;
  intervening_if?: InterveningCondition | null;
  /**
   * This mandatory stack program is placed by the rules, never cast or
   * activated. Its existing Instant shape carries no timing permission.
   */
  program: SpellProgram;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "DeathFilter".
 */
export interface DeathFilter {
  controlled: boolean;
  non_subtypes: string[];
  non_token: boolean;
  other: boolean;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Decklist".
 */
export interface Decklist {
  cards: DeckEntry[];
  /**
   * This physical designation is included in the hundred cards below.
   */
  commander: string;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "DeckEntry".
 */
export interface DeckEntry {
  count: number;
  oracle_id: string;
}
