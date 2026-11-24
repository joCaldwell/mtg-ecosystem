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
 * The intrinsic ability for a basic land type or an index into this source's printed mana abilities.
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
      action: "cast";
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
    };
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
  | "protection_from_everything";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CreatureRestriction".
 */
export type CreatureRestriction = "cant_block" | "cant_be_blocked" | "cant_attack_or_block" | "can_block_only_flying";
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
  handle?: string | null;
  hybrid_colors?: HybridPayment[];
  mana_value?: ExactQuantity | null;
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
