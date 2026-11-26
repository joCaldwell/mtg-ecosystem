/* Generated from Rust mtg-server schema; do not edit. */

/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "DecimalU64Counter".
 */
export type DecimalU64Counter = string;
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ResumeState".
 */
export type ResumeState =
  | {
      status: "pending";
    }
  | {
      observation: Observation;
      status: "available";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Candidate".
 */
export type Candidate =
  | {
      candidate: "object";
      object: string;
    }
  | {
      candidate: "stack";
      stack: string;
    }
  | {
      candidate: "player";
      player: number;
    }
  | {
      candidate: "trigger";
      trigger: string;
    }
  | {
      candidate: "damage_effect";
      damage: string;
      effect: string;
    }
  | {
      candidate: "color";
      color: Color;
    }
  | {
      ability: ManaAbilityIdentity;
      candidate: "mana_ability";
    }
  | {
      candidate: "additional_cost";
      id: string;
      mana: ManaCost;
    }
  | {
      candidate: "modes";
      labels: string[];
      mask: number;
    }
  | {
      candidate: "hybrid_payment";
      payment: HybridPayment;
    }
  | {
      candidate: "flag";
      flag: Flag;
    }
  | {
      action: Action;
      candidate: "action";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Color".
 */
export type Color = "W" | "U" | "B" | "R" | "G" | "C";
/**
 * The intrinsic ability for a basic land type, an index into this source's
 * printed mana abilities, or an index into the mana abilities battlefield
 * statics grant it now (`VisibleObject::granted_mana`, CR 113.10a).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ManaAbilityIdentity".
 */
export type ManaAbilityIdentity =
  | {
      ability: "intrinsic";
      color: Color;
    }
  | {
      ability: "printed";
      index: number;
    }
  | {
      ability: "granted";
      index: number;
    };
/**
 * This interface was referenced by `undefined`'s JSON-Schema definition
 * via the `patternProperty` "^\d+$".
 *
 * This interface was referenced by `undefined`'s JSON-Schema definition
 * via the `patternProperty` "^\d+$".
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ExactQuantity".
 */
export type ExactQuantity = string;
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "HybridPayment".
 */
export type HybridPayment = "pay_two_life" | Color;
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Flag".
 */
export type Flag = "accept" | "decline" | "keep" | "mulligan" | "command_zone" | "destination";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Action".
 */
export type Action =
  | {
      action: "pass";
    }
  | {
      action: "play_land";
      object: string;
    }
  | {
      action: "play_land_back_face";
      object: string;
    }
  | {
      action: "cast";
      card: string;
    }
  | {
      action: "cast_back_face";
      card: string;
    }
  | {
      action: "activate_mana";
      object: string;
    }
  | {
      ability: number;
      action: "activate_ability";
      object: string;
    };
/**
 * Display facts. Bounds and joint constraints are carried by the shape.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Reason".
 */
export type Reason =
  | {
      reason: "priority";
    }
  | {
      reason: "targets";
      specs: TargetSpec[];
    }
  | {
      reason: "mana";
      source: string | null;
    }
  | {
      reason: "attackers";
    }
  | {
      reason: "blockers";
    }
  | {
      power: ExactQuantity;
      reason: "combat_damage";
      source: string;
    }
  | {
      reason: "discard";
    }
  | {
      designation: string;
      destination: Zone;
      object: string | null;
      previous: VisibleCommanderReturnChoice[];
      reason: "commander_return";
    }
  | {
      object: string;
      offered: GraveyardOrderOption | null;
      reason: "graveyard_replacement_order";
    }
  | {
      reason: "optional_effect";
    }
  | {
      reason: "damage_effect";
      recipient: PlayerTargetEntity;
    }
  | {
      reason: "order_triggers";
    }
  | {
      bottom_if_mulligan: number;
      mulligans_taken: number;
      reason: "mulligan";
    }
  | {
      reason: "bottom_opening";
    }
  | {
      reason: "sacrifice";
    }
  | {
      reason: "sacrifice_cost";
    }
  | {
      action: ChosenCostAction;
      reason: "chosen_object_cost";
    }
  | {
      previous: [number, string][];
      reason: "forced_sacrifice";
    }
  | {
      reason: "announce_x";
    }
  | {
      previous: string[];
      reason: "legend_rule";
    }
  | {
      reason: "additional_costs";
    }
  | {
      reason: "choose_modes";
    }
  | {
      reason: "hybrid_cost";
    }
  | {
      reason: "mana_ability";
      source: string;
    }
  | {
      reason: "mana_color";
      source: string | null;
    }
  | {
      reason: "spend_mana";
      source: string | null;
    }
  | {
      reason: "resolve_discard";
    }
  | {
      reason: "search_library";
    }
  | {
      reason: "scry_bottom";
    }
  | {
      reason: "surveil_graveyard";
    }
  | {
      reason: "order_library_top";
    }
  | {
      reason: "order_library_bottom";
    }
  | {
      reason: "entry_attachment";
    }
  | {
      reason: "populate";
    }
  | {
      reason: "proliferate";
    }
  | {
      reason: "resolve_object_choice";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Chooser".
 */
export type Chooser = "controller";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CardKind".
 */
export type CardKind = "land" | "creature" | "instant" | "sorcery" | "enchantment" | "artifact";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * One conjunct of an object predicate. Card types, supertypes and subtypes
 * are the object's current ones (CR 205, 613.1d); colors are current colors
 * (CR 105, 613.1e); power, toughness and keywords are fully derived
 * (CR 613.4, 613.1f).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Atom".
 */
export type Atom =
  | {
      atom: "type";
      kind: CardKind;
    }
  | {
      atom: "not_type";
      kind: CardKind;
    }
  | {
      atom: "any_type";
      kinds: CardKind[];
    }
  | {
      atom: "subtype";
      subtype: string;
    }
  | {
      atom: "not_subtype";
      subtype: string;
    }
  | {
      atom: "legendary";
    }
  | {
      atom: "not_legendary";
    }
  | {
      atom: "basic";
    }
  | {
      atom: "not_basic";
    }
  | {
      atom: "color";
      color: Color;
    }
  | {
      atom: "not_color";
      color: Color;
    }
  | {
      atom: "controlled_by";
      player: PlayerExpr;
    }
  | {
      atom: "owned_by";
      player: PlayerExpr;
    }
  | {
      atom: "tapped";
    }
  | {
      atom: "untapped";
    }
  | {
      atom: "attacking";
    }
  | {
      atom: "blocking";
    }
  | {
      atom: "token";
    }
  | {
      atom: "not_token";
    }
  | {
      atom: "keyword";
      keyword: KeywordAbility;
    }
  | {
      atom: "not_keyword";
      keyword: KeywordAbility;
    }
  | {
      atom: "power";
      cmp: Cmp;
      value: Amount;
    }
  | {
      atom: "toughness";
      cmp: Cmp;
      value: Amount;
    }
  | {
      atom: "mana_value";
      cmp: Cmp;
      value: Amount;
    }
  | {
      atom: "is";
      object: Ref;
    }
  | {
      atom: "is_not";
      object: Ref;
    }
  | {
      alternatives: Pred[];
      atom: "any_of";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "PlayerExpr".
 */
export type PlayerExpr =
  | {
      player: "is";
      who: Ref;
    }
  | {
      object: Ref;
      player: "controller_of";
    }
  | {
      object: Ref;
      player: "owner_of";
    }
  | {
      player: "each_opponent";
    }
  | {
      player: "each_player";
    };
/**
 * The closed reference vocabulary. Every operand that is not a literal is
 * built from these, and the reference binder emits exactly these.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Ref".
 */
export type Ref =
  | {
      ref: "source";
    }
  | {
      ref: "controller";
      role: ControllerRole;
    }
  | {
      ref: "target";
      slot: number;
    }
  | {
      ref: "attached_to";
    }
  | {
      ref: "event_object";
      role: EventObjectRole;
    }
  | {
      ref: "event_player";
      role: EventPlayerRole;
    }
  | {
      ref: "entering_object";
    }
  | {
      ref: "step_result";
      role: StepRole;
      step: number;
    }
  | {
      id: number;
      ref: "bound_var";
    }
  | {
      id: number;
      ref: "linked_pool";
    }
  | {
      ref: "announced_x";
    };
/**
 * Whose "you" a controller reference means (CR 109.5). The role is fixed by
 * where the text is printed, so a reference states it instead of leaving the
 * reader to infer it from the surrounding ability.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ControllerRole".
 */
export type ControllerRole =
  | "spell_controller"
  | "ability_controller"
  | "static_controller"
  | "entering_controller"
  | "prospective_caster"
  | "special_action_player"
  | "card_owner"
  | "ability_holder";
/**
 * The object an event reference names (CR 603.2, 603.10).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "EventObjectRole".
 */
export type EventObjectRole =
  | "entered"
  | "died"
  | "dealt_damage"
  | "damaged"
  | "cast_spell"
  | "tapped_for_mana"
  | "attacked"
  | "blocked"
  | "sacrificed"
  | "left_battlefield";
/**
 * The player an event reference names.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "EventPlayerRole".
 */
export type EventPlayerRole =
  | ("damaged_player" | "controller_of_event_object")
  | "caster"
  | "activating_player"
  | "drawing_player"
  | "defending_player"
  | "active_player";
/**
 * What an earlier step of the same node produced.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "StepRole".
 */
export type StepRole = "objects" | "player" | "amount";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
  | "protection_from_everything"
  | "intimidate"
  | "shadow"
  | "horsemanship"
  | "fear"
  | "skulk";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Cmp".
 */
export type Cmp = "at_least" | "at_most" | "exactly";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Amount".
 */
export type Amount =
  | {
      amount: "fixed";
      value: ExactQuantity;
    }
  | {
      amount: "var";
      of: Ref;
    }
  | {
      amount: "count";
      set: SetExpr;
    }
  | {
      amount: "stat_of";
      object: Ref;
      stat: Stat;
    }
  | {
      amount: "greatest";
      set: SetExpr;
      stat: Stat;
    }
  | {
      amount: "times";
      factor: ExactQuantity;
      of: Amount;
    }
  | {
      amount: "devotion";
      colors: Color[];
      player: PlayerExpr;
    }
  | {
      amount: "life_of";
      player: PlayerExpr;
    }
  | {
      amount: "other_casts_mana_value";
      caster: PlayerExpr;
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "SetExpr".
 */
export type SetExpr =
  | {
      object: Ref;
      set: "one";
    }
  | {
      filter: Pred;
      set: "matching";
      zone: ZoneExpr;
    }
  | {
      pool: Ref;
      set: "pool";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ZoneExpr".
 */
export type ZoneExpr =
  | {
      zone: "battlefield";
    }
  | {
      zone: "graveyards";
    }
  | {
      of: PlayerExpr;
      zone: "graveyard";
    }
  | {
      of: PlayerExpr;
      zone: "hand";
    }
  | {
      of: PlayerExpr;
      zone: "library";
    }
  | {
      zone: "exile";
    }
  | {
      zone: "stack";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Stat".
 */
export type Stat = "power" | "toughness" | "mana_value";
/**
 * A conjunction of atoms. The empty predicate admits every object of the
 * zone it is evaluated in.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Pred".
 */
export type Pred = Atom[];
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "EvaluationStage".
 */
export type EvaluationStage = "announcement";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "GraveyardOrderOption".
 */
export type GraveyardOrderOption = "regenerate";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ChosenCostAction".
 */
export type ChosenCostAction = "discard" | "tap" | "exile_from_graveyard";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Shape".
 */
export type Shape =
  | {
      rules: JointRule[];
      shape: "pick";
      slots: Slot[];
    }
  | {
      items: number[];
      max: number;
      min: number;
      shape: "order";
    }
  | {
      buckets: Bucket[];
      gates: Gate[];
      shape: "distribute";
      total: ExactQuantity;
    }
  | {
      max: ExactQuantity | null;
      min: ExactQuantity;
      shape: "number";
    }
  | {
      cost: ManaCost;
      pool: ManaPool;
      shape: "pay";
      step: PayStep;
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "JointRule".
 */
export type JointRule =
  | {
      rule: "distinct";
      slots: number[];
    }
  | {
      candidates: number[];
      count: number;
      rule: "picked_at_least";
    }
  | {
      candidates: number[];
      count: number;
      rule: "picked_at_most";
    }
  | {
      max: number;
      min: number;
      rule: "total";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "SlotRole".
 */
export type SlotRole =
  | {
      role: "choice";
    }
  | {
      role: "target";
      spec: number;
    }
  | {
      role: "attack_with";
    }
  | {
      role: "block_with";
    }
  | {
      index: number;
      role: "hybrid_symbol";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "PayStep".
 */
export type PayStep =
  | {
      sources: number[];
      step: "produce";
    }
  | {
      step: "spend";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "DecisionSourceKind".
 */
export type DecisionSourceKind = "spell" | "activated" | "triggered" | "mana_ability" | "effect";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleEffect".
 */
export type VisibleEffect =
  | {
      effect: "regeneration_shield";
      expires_in_cleanup_of_turn: number;
      handle: string;
      origin: VisibleEffectSource;
      target: VisibleTarget;
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
      affects: CombatProhibitionScope;
      effect: "combat_prohibition";
      expires_in_cleanup_of_turn: number;
      handle: string;
      origin: VisibleEffectSource;
      restriction: CombatRequirement;
    }
  | {
      controller: number;
      effect: "gain_control";
      expires_in_cleanup_of_turn: number | null;
      handle: string;
      origin: VisibleEffectSource;
      targets: VisibleTarget[];
      while_source_remains: boolean;
    }
  | {
      effect: "set_characteristics";
      expires_in_cleanup_of_turn: number;
      handle: string;
      origin: VisibleEffectSource;
      targets: VisibleTarget[];
      values: CharacteristicSet;
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
    };
/**
 * CR 611.2c: a rules restriction can name one exact object or apply to a
 * live class of creatures. It does not grant or remove an ability.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CombatProhibitionScope".
 */
export type CombatProhibitionScope =
  | {
      scope: "target";
      target: VisibleTarget;
    }
  | {
      scope: "creatures";
    }
  | {
      scope: "creatures_without_flying";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CombatRequirement".
 */
export type CombatRequirement =
  | (
      | "cant_attack"
      | "cant_block"
      | "cant_attack_or_block"
      | "cant_be_blocked"
      | "can_block_only_flying"
      | "attack_each_combat"
    )
  | "attack_as_though_no_defender";
/**
 * CR 712.8a,d-f and 715.4: a two-part card has its front (or normal)
 * characteristics until its back face (or Adventure) is chosen for the
 * stack or battlefield, or the permanent transforms.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CardFace".
 */
export type CardFace = "front" | "back";
/**
 * How the two printed parts of one physical card relate (CR 712, 715).
 * <https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "FaceLayout".
 */
export type FaceLayout = "single" | "modal_double_faced" | "transforming" | "adventurer";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Cond".
 */
export type Cond =
  | {
      cmp: Cmp;
      cond: "compare";
      left: Amount;
      right: Amount;
    }
  | {
      cond: "active_player";
      player: PlayerExpr;
    }
  | {
      cond: "exists";
      set: SetExpr;
    }
  | {
      cond: "cost_paid";
      identity: string;
    }
  | {
      cond: "satisfies";
      filter: Pred;
      object: Ref;
    }
  | {
      caster: PlayerExpr;
      cond: "cast_by";
      object: Ref;
    }
  | {
      cond: "controls_own_commander";
    }
  | {
      cond: "opponent_controls_more";
      kind: CardKind;
    }
  | {
      cond: "not";
      condition: Cond;
    }
  | {
      cond: "all";
      conditions: Cond[];
    };
/**
 * CR 106.1, 106.4: the mana one resolving mana ability adds.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
    }
  | {
      amount: ExactQuantity;
      colors: Color[];
      mana: "choose";
    }
  | {
      colors_only: boolean;
      controller: ManaLandController;
      mana: "lands_could_produce";
    };
/**
 * Whose lands a "could produce" ability inspects, relative to the mana
 * ability's controller (CR 109.5, 102.3).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ManaLandController".
 */
export type ManaLandController = "you" | "opponents";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ManaActivationRestriction".
 */
export type ManaActivationRestriction = "once_each_turn";
/**
 * Control over verbs. A node is performed in written order (CR 608.2c).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Node".
 */
export type Node =
  | {
      do: Verb;
    }
  | {
      seq: Node[];
    }
  | {
      if: {
        condition: Cond;
        otherwise?: Node | null;
        then: Node;
      };
    }
  | {
      optional: {
        chooser: PlayerExpr;
        node: Node;
      };
    }
  | {
      for_each: {
        body: Node;
        over: Domain;
        var: number;
      };
    }
  | {
      choose: {
        body: Node;
        chooser: PlayerExpr;
        from: SetExpr;
        maximum: Amount;
        minimum: Amount;
        var: number;
      };
    }
  | {
      choose_object: {
        action: ObjectChoiceAction;
        from: SetExpr;
      };
    }
  | {
      tap_targets: {
        count: number;
        tap: boolean;
      };
    }
  | {
      remove_targets: {
        count: number;
        removal: GroupRemoval;
      };
    }
  | {
      unless_pays: {
        node: Node;
        payer: PlayerExpr;
        payment: Payment;
      };
    }
  | {
      if_pays: {
        node: Node;
        payer: PlayerExpr;
        payment: Payment;
      };
    }
  | {
      delayed: {
        event: DelayedEvent;
        program: SpellProgram;
      };
    }
  | {
      reflexive: {
        payment: ManaCost;
        program: SpellProgram;
      };
    };
/**
 * One game action. A verb acts on every member of its operands as one
 * simultaneous event unless its rule says otherwise (CR 608.2c, 101.4).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Verb".
 */
export type Verb =
  | {
      amount: Amount;
      player: PlayerExpr;
      verb: "draw";
    }
  | {
      amount: Amount;
      player: PlayerExpr;
      verb: "gain_life";
    }
  | {
      amount: Amount;
      player: PlayerExpr;
      verb: "lose_life";
    }
  | {
      first: Ref;
      second: Ref;
      verb: "fight";
    }
  | {
      player: PlayerExpr;
      verb: "populate";
    }
  | {
      player: PlayerExpr;
      verb: "proliferate";
    }
  | {
      amount: Amount;
      source: Ref;
      to: DamageTo;
      verb: "damage";
    }
  | {
      /**
       * CR 701.19c.
       */
      regeneration_forbidden?: boolean;
      verb: "destroy";
      what: SetExpr;
    }
  | {
      to: Destination;
      verb: "move";
      what: SetExpr;
    }
  | {
      verb: "tap";
      what: SetExpr;
    }
  | {
      verb: "untap";
      what: SetExpr;
    }
  | {
      amount: Amount;
      counter: PlayerCounterKind;
      player: PlayerExpr;
      verb: "give_counters";
    }
  | {
      amount: Amount;
      counter: CounterKind;
      on: SetExpr;
      verb: "put_counters";
    }
  | {
      controller: PlayerExpr;
      count: Amount;
      /**
       * CR 111.1, 614.1c: the tokens enter tapped.
       */
      tapped?: boolean;
      token: TokenSpec;
      verb: "create_tokens";
    }
  | {
      amount: Amount;
      player: PlayerExpr;
      verb: "mill";
    }
  | {
      amount: Amount;
      player: PlayerExpr;
      random?: boolean;
      verb: "discard";
    }
  | {
      count: Amount;
      filter: Pred;
      player: PlayerExpr;
      verb: "sacrifice";
    }
  | {
      mana: ManaSpec;
      player: PlayerExpr;
      verb: "add_mana";
    }
  | {
      player: PlayerExpr;
      search: SearchSpec;
      verb: "search";
    }
  | {
      verb: "counter";
      what: Ref;
    }
  | {
      controller: PlayerExpr;
      until: Until;
      verb: "gain_control";
      what: SetExpr;
    }
  | {
      until: Until;
      values: CharacteristicSet;
      verb: "set_characteristics";
      what: SetExpr;
    }
  | {
      to: Ref;
      verb: "attach";
      what: Ref;
    }
  | {
      change: Change;
      layer: Layer;
      until: Until;
      verb: "modify_until";
      what: SetExpr;
    }
  | {
      affects: CombatProhibitionScope2;
      restriction: CombatRequirement;
      until: Until;
      verb: "restrict_combat";
    }
  | {
      verb: "return_captured_exile";
    }
  | {
      event: DelayedEvent;
      node: Node;
      verb: "create_delayed_trigger";
    }
  | {
      verb: "copy_stack_object";
      what: Ref;
    }
  | {
      permission: Permission;
      player: PlayerExpr;
      until: Until;
      verb: "play_permission";
      what: SetExpr;
    }
  | {
      verb: "transform";
      what: Ref;
    }
  | {
      designation: Designation;
      player: PlayerExpr;
      verb: "set_designation";
    }
  | {
      amount: Amount;
      player: PlayerExpr;
      verb: "reorder_library_top";
    }
  | {
      amount: Amount;
      player: PlayerExpr;
      verb: "scry";
    }
  | {
      amount: Amount;
      player: PlayerExpr;
      verb: "surveil";
    }
  | {
      player: PlayerExpr;
      verb: "shuffle";
    }
  | {
      verb: "regenerate";
      what: SetExpr;
    }
  | {
      id: BuiltinId;
      verb: "builtin";
    };
/**
 * Who or what damage is dealt to (CR 120.1-120.3).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "DamageTo".
 */
export type DamageTo =
  | {
      to: "ref";
      who: Ref;
    }
  | {
      players: PlayerExpr;
      to: "players";
    }
  | {
      set: SetExpr;
      to: "objects";
    }
  | {
      parts: DamageTo[];
      to: "all";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Destination".
 */
export type Destination =
  | {
      to: "exile";
    }
  | {
      to: "owner_hand";
    }
  | {
      to: "owner_library_top";
    }
  | {
      to: "owner_library_bottom";
    }
  | {
      to: "owner_graveyard";
    }
  | {
      controller: PlayerExpr;
      tapped?: boolean;
      to: "battlefield";
    };
/**
 * Player counters for poison and energy (CR 107.14, 122.1f, 704.5c).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "PlayerCounterKind".
 */
export type PlayerCounterKind = "poison" | "energy";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CounterKind".
 */
export type CounterKind =
  | {
      counter: "plus_one_plus_one";
    }
  | {
      counter: "minus_one_minus_one";
    }
  | {
      counter: "named";
      name: string;
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "TokenSpec".
 */
export type TokenSpec =
  | {
      spec: CreatureToken;
      token: "creature";
    }
  | {
      kind: PredefinedToken;
      token: "predefined";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "PredefinedToken".
 */
export type PredefinedToken = "treasure" | "gold" | "food" | "clue";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ManaSpec".
 */
export type ManaSpec =
  | {
      amount: Amount;
      color: Color;
      mana: "color";
    }
  | {
      amount: Amount;
      colors: Color[];
      mana: "any_combination";
    };
/**
 * How long a continuous effect lasts (CR 611.2a-b).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Until".
 */
export type Until = "end_of_combat" | "end_of_turn" | "controller_next_turn" | "source_leaves" | "indefinite";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Change".
 */
export type Change =
  | {
      change: "power_toughness";
      power: Amount;
      toughness: Amount;
    }
  | {
      change: "power_toughness_x";
      power: PumpDelta;
      toughness: PumpDelta;
    }
  | {
      change: "set_power_toughness";
      power: Amount;
      toughness: Amount;
    }
  | {
      change: "add_keywords";
      keywords: KeywordAbility[];
    }
  | {
      change: "remove_keywords";
      keywords: KeywordAbility[];
    }
  | {
      abilities: ManaAbility[];
      change: "add_mana_abilities";
    }
  | {
      change: "set_colors";
      colors: Color[];
    }
  | {
      change: "add_types";
      kinds: CardKind[];
    }
  | {
      change: "add_creature_subtypes";
      subtypes: string[];
    }
  | {
      change: "all_creature_types";
    }
  | {
      change: "controller";
      player: PlayerExpr;
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "PumpDelta".
 */
export type PumpDelta =
  | {
      delta: "fixed";
      value: ExactQuantity;
    }
  | {
      delta: "x";
      negative: boolean;
    }
  | {
      amount: Amount;
      delta: "counted";
      negative: boolean;
    };
/**
 * CR 613.1: the layer a continuous change applies in.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Layer".
 */
export type Layer =
  "copy" | "control" | "text" | "type" | "color" | "ability" | "power_toughness_set" | "power_toughness_modify";
/**
 * CR 611.2c: a rules restriction can name one exact object or apply to a
 * live class of creatures. It does not grant or remove an ability.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CombatProhibitionScope2".
 */
export type CombatProhibitionScope2 =
  | {
      scope: "target";
      target: Ref;
    }
  | {
      scope: "creatures";
    }
  | {
      scope: "creatures_without_flying";
    };
/**
 * CR 603.7: when a delayed triggered ability triggers.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "DelayedEvent".
 */
export type DelayedEvent =
  | {
      at: "next_end_step";
    }
  | {
      at: "next_turn_upkeep";
    }
  | {
      at: "next_end_step_after_exile";
      slot: number;
    }
  | {
      at: "target_leaves_after_exiling_source";
      slot: number;
    }
  | {
      at: "next_upkeep";
      of: PlayerExpr;
    }
  | {
      at: "end_of_combat";
    }
  | {
      at: "event";
      event: TriggerCondition;
      until: Until;
    };
/**
 * CR 603.2: an ordinary triggered ability's event, and the intervening
 * "if" it checks when the event occurs and again as it resolves (CR 603.4).
 * [`kir::check_event`] admits the pattern and [`kir::Intervening::of`] the
 * condition.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "TriggerCondition".
 */
export type TriggerCondition = {
  event: {
    intervening_if?: Cond | null;
    /**
     * "This ability triggers only once each turn": each incarnation of
     * the source triggers this ability at most once per turn (CR 400.7).
     */
    once_each_turn?: boolean;
    pattern: EventPattern;
  };
};
/**
 * One event an ordinary triggered ability triggers on (CR 603.2).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "EventPattern".
 */
export type EventPattern =
  | {
      event: "enters";
      object: Pred;
    }
  | {
      event: "dies";
      object: Pred;
    }
  | {
      event: "becomes_tapped";
      object: Pred;
    }
  | {
      attacker: Pred;
      event: "attacks";
    }
  | {
      attacker: Pred;
      event: "attacks_unblocked";
    }
  | {
      caster: PlayerExpr;
      event: "cast";
      nth?: TurnOrdinal | null;
      spell: Pred;
      /**
       * CR 115.1, 601.2c: "a spell that targets this [permanent]": the
       * source is among the permanents the spell targets as it becomes
       * cast.
       */
      targets_source?: boolean;
    }
  | {
      by: PlayerExpr;
      event: "becomes_target";
      kind: TargetingKind;
      object: Pred;
    }
  | {
      event: "draws";
      nth?: TurnOrdinal | null;
      player: PlayerExpr;
    }
  | {
      event: "gains_life";
      nth?: TurnOrdinal | null;
      player: PlayerExpr;
    }
  | {
      event: "sacrifices";
      object: Pred;
      player: PlayerExpr;
    }
  | {
      event: "discards";
      player: PlayerExpr;
    }
  | {
      event: "leaves_battlefield";
      object: Pred;
    }
  | {
      blocker: Pred;
      event: "blocks";
    }
  | {
      attacker: Pred;
      event: "becomes_blocked";
    }
  | {
      combat: boolean;
      event: "is_dealt_damage";
      object: Pred;
    }
  | {
      active: PlayerExpr;
      event: "step_begins";
      step: BeginningStep;
    }
  | {
      combat: boolean;
      event: "deals_damage";
      recipient: PlayerExpr;
      source: Pred;
    }
  | {
      event: "any_of";
      first: EventPattern;
      second: EventPattern;
    };
/**
 * CR 115.1: the kind of stack object that announced a target.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "TargetingKind".
 */
export type TargetingKind = "spell" | "ability" | "spell_or_ability";
/**
 * Exact step predicates; delayed and conditional boundaries are excluded.
 * CR 503-507, 511-513: the beginning of the upkeep, draw, beginning of
 * combat, end of combat and end steps, and of the precombat and postcombat
 * main phases. A turn has one combat phase, so its postcombat main phase is
 * its second main phase (CR 505.1a).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "BeginningStep".
 */
export type BeginningStep =
  "precombat_main" | "upkeep" | "draw" | "end" | "beginning_of_combat" | "end_of_combat" | "postcombat_main";
/**
 * What a play permission allows (CR 601.3, 305.1).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Permission".
 */
export type Permission = "play" | "cast" | "cast_without_paying_mana_cost";
/**
 * A designation a player or object carries (CR 700.6, 725, 726).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Designation".
 */
export type Designation = "monarch" | "initiative" | "city_blessing" | "monstrous" | "renowned";
/**
 * A whole reviewed procedure the algebra does not decompose. Each id names
 * one Comprehensive Rules keyword action, never a card.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "BuiltinId".
 */
export type BuiltinId = "proliferate" | "cascade" | "demonstrate" | "hideaway";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Domain".
 */
export type Domain =
  | {
      over: "players";
      players: PlayerExpr;
    }
  | {
      over: "objects";
      set: SetExpr;
    };
/**
 * The instruction performed on a single public object selected at resolution.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ObjectChoiceAction".
 */
export type ObjectChoiceAction = "return_to_owner_hand";
/**
 * The simultaneous battlefield removal a counted target group performs.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "GroupRemoval".
 */
export type GroupRemoval = "destroy" | "exile" | "owner_hand";
/**
 * CR 118: what a player may pay during resolution.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Payment".
 */
export type Payment =
  | {
      amount: Amount;
      cost: "generic";
    }
  | {
      amount: Amount;
      cost: "life";
    }
  | {
      cost: "mana";
      mana: ManaCost;
    }
  | {
      cost: "sacrifice_creature";
    }
  | {
      cost: "discard_artifact";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "FunctioningZone".
 */
export type FunctioningZone = ("hand" | "graveyard" | "battlefield" | "stack") | "any";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Instruction".
 */
export type Instruction = {
  node: Node;
  op: "kernel";
  source_block: number;
};
/**
 * Permanent resolution is distinct from an empty instruction program.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "SpellResolution".
 */
export type SpellResolution = "instructions" | "permanent";
/**
 * Supported announcement timing; permissions are checked by the runtime.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "SpellTiming".
 */
export type SpellTiming = "instant" | "main_phase";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CreatureRestriction".
 */
export type CreatureRestriction =
  "cant_attack" | "cant_block" | "cant_be_blocked" | "cant_attack_or_block" | "can_block_only_flying";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "PlayerStatus".
 */
export type PlayerStatus = "live" | "left";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleSetupPhase".
 */
export type VisibleSetupPhase = "declare" | "bottom";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * Public origin uses only already captured public/historical handles.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleTriggerAbility".
 */
export type VisibleTriggerAbility =
  | {
      index: number;
      origin: "intrinsic";
    }
  | {
      origin: "followup";
    };
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * One coherent committed own-stream boundary. No query is run to refresh it.
 */
export interface SeatResume {
  catalog_sha256: string;
  engine_build_id: string;
  /**
   * Last published lifecycle, not the worker's private scheduling phase.
   */
  lifecycle: ExecutionStatus | null;
  match_id: string;
  /**
   * Last included own publication cursor; poll strictly after this value.
   */
  next: DecimalU64Counter;
  outcome: RulesResult | null;
  protocol_version: number;
  server_build_id: string;
  state: ResumeState;
  viewer: number;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Observation".
 */
export interface Observation {
  decision: Decision | null;
  decision_context?: DecisionContext | null;
  observation_version: number;
  view: PermittedView;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Decision".
 */
export interface Decision {
  candidates: Candidate[];
  player: number;
  reason: Reason;
  shape: Shape;
  token: string;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ManaCost".
 */
export interface ManaCost {
  colored: Color[];
  generic: ExactQuantity;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
   * Additional fixed characteristic tests at announcement and resolution.
   */
  predicate?: Pred | null;
  /**
   * Require the current battlefield incarnation to be tapped (CR 110.5,
   * 601.2c, 608.2b). False imposes no tap-status restriction.
   */
  tapped?: boolean;
  types: CardKind[];
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleCommanderReturnChoice".
 */
export interface VisibleCommanderReturnChoice {
  designation: string;
  owner: number;
  to_command: boolean;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Slot".
 */
export interface Slot {
  allowed: number[];
  max: number;
  min: number;
  repeat: boolean;
  role: SlotRole;
  subject: number | null;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Bucket".
 */
export interface Bucket {
  candidate: number;
  max: ExactQuantity;
  min: ExactQuantity;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Gate".
 */
export interface Gate {
  /**
   * Bucket ordinal whose positive allocation depends on the requirements.
   */
  bucket: number;
  requires: [number, ExactQuantity][];
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ManaPool".
 */
export interface ManaPool {
  unrestricted: {
    B?: ExactQuantity;
    C?: ExactQuantity;
    G?: ExactQuantity;
    R?: ExactQuantity;
    U?: ExactQuantity;
    W?: ExactQuantity;
  };
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "DecisionContext".
 */
export interface DecisionContext {
  candidates?: CandidateLabel[];
  instruction?: DecisionInstruction | null;
  source?: DecisionSource | null;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CandidateLabel".
 */
export interface CandidateLabel {
  source?: DecisionSource | null;
  text: string;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "DecisionInstruction".
 */
export interface DecisionInstruction {
  source_block: number;
  text: string;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleAttacker".
 */
export interface VisibleAttacker {
  attacker: string;
  blocked: boolean;
  blockers: string[];
  defender: number;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleCommander".
 */
export interface VisibleCommander {
  designation: string;
  history: CommanderHistory;
  oracle_id: string;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleEffectSource".
 */
export interface VisibleEffectSource {
  controller: number;
  oracle_id: string;
  source: VisibleTarget;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleTarget".
 */
export interface VisibleTarget {
  announced: string;
  current: string | null;
}
/**
 * Fixed signed power/toughness changes belonging to one complete creature effect.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CreatureStatModifier".
 */
export interface CreatureStatModifier {
  power: ExactQuantity;
  toughness: ExactQuantity;
}
/**
 * Fixed values installed by one resolving effect (CR 611.2c, 613.6).
 * An omitted characteristic retains its value from the other effects.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CharacteristicSet".
 */
export interface CharacteristicSet {
  animation?: Animation | null;
  base_power?: ExactQuantity | null;
  base_toughness?: ExactQuantity | null;
  colors?: Color[] | null;
  keywords?: KeywordAbility[];
}
/**
 * CR 205.1b: this permanent becomes a creature while retaining its
 * card types and supertypes. Only its creature subtypes can be replaced.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Animation".
 */
export interface Animation {
  add_artifact: boolean;
  creature_subtypes: string[];
  replace_creature_subtypes: boolean;
}
/**
 * A remembered public tuck. Position is one-based; memory gives no object
 * handle or authority to inspect or target a card in a hidden library.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "KnownLibraryPosition".
 */
export interface KnownLibraryPosition {
  library_owner: number;
  oracle_id: string;
  position: number;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleObject".
 */
export interface VisibleObject {
  attached_to?: string | null;
  attachments?: string[];
  attack_ready: boolean;
  characteristics?: Characteristics | null;
  controller: number;
  entered_this_turn?: boolean;
  exiled_by?: string | null;
  face_down?: boolean;
  faces?: VisibleFaces | null;
  /**
   * CR 113.10a, 613.1f: mana abilities battlefield statics grant this
   * permanent now, in timestamp order; public like its keywords.
   */
  granted_mana?: ManaAbility[];
  handle: string;
  keywords?: KeywordAbility[];
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "Characteristics".
 */
export interface Characteristics {
  /**
   * Other simultaneous card types; empty preserves existing catalog bytes.
   */
  additional_types?: CardKind[];
  /**
   * CR 205.3m, 702.73a: this object is every creature type. Printed
   * changeling and "is every creature type" characteristic-defining
   * abilities set it in every zone (CR 604.3); layer 4 effects set or
   * clear it (CR 613.1d). Omitted when false.
   */
  all_creature_types?: boolean;
  basic: boolean;
  color_identity: Color[];
  colors: Color[];
  /**
   * CR 202.3b, 712.8e: the back face of a nonmodal double-faced card has
   * no mana cost, but its mana value is calculated from its front face's
   * mana cost. Present only on such a back face.
   */
  front_face_mana_value?: ExactQuantity | null;
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * The two printed parts of a double-faced or adventurer card. Available
 * only when the viewer can identify the card (CR 712.6-7).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleFaces".
 */
export interface VisibleFaces {
  /**
   * The face up, or the Adventure while cast as one (CR 712.8, 715.3b).
   */
  active_face: CardFace;
  /**
   * The back face, or an adventurer's Adventure (CR 715.2).
   */
  back_name: string;
  front_name: string;
  layout: FaceLayout;
  /**
   * CR 715.3d: exiled as a resolving Adventure; its controller may play it.
   */
  on_adventure?: boolean;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ManaAbility".
 */
export interface ManaAbility {
  /**
   * CR 602.5, 605.3a: "Activate only if"; one
   * [`kir::ActivationCondition`] read as the activation begins.
   */
  condition?: Cond | null;
  costs: PermanentActivationCosts;
  production: ManaProduction;
  restriction?: ManaActivationRestriction | null;
  /**
   * CR 605.1a, 605.3b: the ability's other printed instructions, Kernel
   * nodes performed in order after the mana is added, while the ability
   * resolves immediately and without the stack. A mana ability has no
   * targets; [`kir::mana_rider`] admits each node.
   */
  riders?: Node[];
  source_block: number;
}
/**
 * Finite permanent costs; costs are committed only after complete payment.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "PermanentActivationCosts".
 */
export interface PermanentActivationCosts {
  mana?: ManaCost | null;
  /**
   * CR 118.3, 119.4: "Pay N life", paid only by a player whose life total
   * is at least N; paying life is loss of life, never damage.
   */
  pay_life?: ExactQuantity | null;
  remove_source_plus_one_counters?: ExactQuantity | null;
  sacrifice: boolean;
  tap: boolean;
}
/**
 * CR 111.3–4: precisely defined creature tokens, never copied card definitions.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CreatureToken".
 */
export interface CreatureToken {
  additional_types?: CardKind[];
  /**
   * CR 702.73a: the token has changeling.
   */
  all_creature_types?: boolean;
  colors: Color[];
  keywords: KeywordAbility[];
  power: ExactQuantity;
  subtypes: string[];
  toughness: ExactQuantity;
}
/**
 * CR 701.23: what a search looks for and where the found cards go. An
 * empty filter is a search "simply for a quantity of cards": the searcher
 * must find `maximum`, or as many as the library holds (CR 701.23d). With a
 * stated quality they may find any number up to `maximum` (CR 701.23b). The
 * found cards move together to `to`; `reveal` shows them first (CR 701.23e).
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "SearchSpec".
 */
export interface SearchSpec {
  filter: Pred;
  maximum: Amount;
  reveal?: boolean;
  to: Destination;
}
/**
 * CR 603.2: "your/their Nth [filtered] spell/card each turn", or with
 * `any_player` "the Nth [filtered] spell of a turn": the event triggers only
 * when it is the `nth` matching event of the current turn, counted from the
 * turn's start whether or not the source was on the battlefield then.
 * Events of the event's player count, or of every player with `any_player`.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "TurnOrdinal".
 */
export interface TurnOrdinal {
  any_player?: boolean;
  nth: number;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "SpellProgram".
 */
export interface SpellProgram {
  additional_sacrifice?: SacrificeFilter | null;
  functioning_zone: FunctioningZone;
  instructions: Instruction[];
  optional_additional_costs?: OptionalAdditionalManaCost[];
  /**
   * CR 115.1d, 601.2c: the sole announced battlefield occurrence may be omitted.
   */
  optional_target?: boolean;
  resolution: SpellResolution;
  targets: TargetSpec[];
  timing: SpellTiming;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * CR 702.33a/e: one source-bound optional fixed mana declaration. The
 * identity names the exact archived keyword; conditions cannot link by position.
 *
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "OptionalAdditionalManaCost".
 */
export interface OptionalAdditionalManaCost {
  identity: string;
  mana: ManaCost;
  source_block: number;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleToken".
 */
export interface VisibleToken {
  keywords: KeywordAbility[];
  name: string;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisiblePlayer".
 */
export interface VisiblePlayer {
  energy_counters?: ExactQuantity;
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
  poison_counters?: ExactQuantity;
  status: PlayerStatus;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleSetup".
 */
export interface VisibleSetup {
  phase: VisibleSetupPhase;
  players: VisibleSetupPlayer[];
  round: number;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleStackEntry".
 */
export interface VisibleStackEntry {
  ability?: VisibleAbility | null;
  cast_complete: boolean;
  controller: number;
  entity_targets?: VisibleEntityTarget[];
  /**
   * CR 702.34a: this spell was cast from its owner's graveyard for its
   * flashback cost and is exiled as it leaves the stack.
   */
  flashback?: boolean;
  handle?: string | null;
  hybrid_colors?: HybridPayment[];
  mana_value?: ExactQuantity | null;
  /**
   * Bit zero is the first printed mode. Absent until modes are announced.
   */
  mode_mask?: number | null;
  player_targets?: number[];
  selected_additional_costs?: string[] | null;
  spell: string | null;
  spell_copy?: boolean;
  stack_targets?: VisibleStackTarget[];
  targets: VisibleTarget[];
  triggered?: VisibleTriggered | null;
  x?: ExactQuantity | null;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleAbility".
 */
export interface VisibleAbility {
  ability: number;
  handle: string;
  oracle_id: string;
  source: VisibleTarget;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleStackTarget".
 */
export interface VisibleStackTarget {
  announced: string;
  current: string | null;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleTriggered".
 */
export interface VisibleTriggered {
  ability: VisibleTriggerAbility;
  handle: string;
  oracle_id: string;
  source: VisibleTarget;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "TurnState".
 */
export interface TurnState {
  lands_played: number;
  number: number;
  step: TurnStep;
}
