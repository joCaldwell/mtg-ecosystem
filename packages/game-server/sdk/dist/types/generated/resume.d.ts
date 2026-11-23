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
 * via the `definition` "PlayerRequest".
 */
export type PlayerRequest =
  | {
      candidates: string[];
      player: number;
      previous_choices: string[];
      request: "legend_rule";
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
      colors: Color[];
      mana_ability_source?: string | null;
      player: number;
      request: "mana_color";
      token: string;
    }
  | {
      candidates: VisibleSearchLand[];
      maximum: number;
      player: number;
      request: "search_lands";
      token: string;
    }
  | {
      player: number;
      request: "optional_effect";
      token: string;
    }
  | {
      player: number;
      request: "optional_recursion";
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
      candidates: VisibleDamageAllocationCandidate[];
      effect: string;
      player: number;
      recipient: PlayerTargetEntity;
      remaining: ExactQuantity;
      request: "damage_allocation";
      required: ExactQuantity;
      token: string;
    }
  | {
      candidates: number[];
      player: number;
      request: "modes";
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "ExactQuantity".
 */
export type ExactQuantity = string;
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
 * via the `definition` "DamageEffectKind".
 */
export type DamageEffectKind = "double_damage" | "prevent_combat_damage" | "prevent_next_damage";
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CardKind".
 */
export type CardKind = "land" | "creature" | "instant" | "sorcery" | "enchantment" | "artifact";
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
 * via the `definition` "DecisionSourceKind".
 */
export type DecisionSourceKind = "spell" | "activated" | "triggered" | "mana_ability" | "effect";
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleEffect".
 */
export type VisibleEffect =
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
      effect: "prevent_next_damage";
      expires_in_cleanup_of_turn: number;
      handle: string;
      origin: VisibleEffectSource;
      remaining: ExactQuantity;
      target: VisibleEntityTarget;
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
  | "menace";
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "CreatureRestriction".
 */
export type CreatureRestriction = "cant_block" | "cant_be_blocked" | "cant_attack_or_block";
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
  decision: PlayerRequest | null;
  decision_context?: DecisionContext | null;
  observation_version: number;
  view: PermittedView;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleSearchLand".
 */
export interface VisibleSearchLand {
  handle: string;
  oracle_id: string;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleDamageAllocationCandidate".
 */
export interface VisibleDamageAllocationCandidate {
  amount: ExactQuantity;
  damage: string;
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisibleDamageEffectCandidate".
 */
export interface VisibleDamageEffectCandidate {
  allocation_required?: boolean;
  amount_after: ExactQuantity;
  amount_before: ExactQuantity;
  budget_remaining: ExactQuantity | null;
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "VisiblePendingTrigger".
 */
export interface VisiblePendingTrigger {
  ability: number;
  handle: string;
  label?: string | null;
  oracle_id: string;
  source: VisibleTarget;
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
   * Require the current battlefield incarnation to be tapped (CR 110.5,
   * 601.2c, 608.2b). False imposes no tap-status restriction.
   */
  tapped?: boolean;
  types: CardKind[];
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
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "LegalBlockers".
 */
export interface LegalBlockers {
  attackers: string[];
  blocker: string;
}
/**
 * This interface was referenced by `SeatResume`'s JSON-Schema
 * via the `definition` "DecisionContext".
 */
export interface DecisionContext {
  instruction?: DecisionInstruction | null;
  modes?: DecisionMode[];
  source?: DecisionSource | null;
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
 * via the `definition` "DecisionMode".
 */
export interface DecisionMode {
  mode: number;
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
 * via the `definition` "PermittedView".
 */
export interface PermittedView {
  active: number | null;
  combat: VisibleAttacker[];
  commanders: VisibleCommander[];
  effects?: VisibleEffect[];
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
 * via the `definition` "VisibleObject".
 */
export interface VisibleObject {
  attached_to?: string | null;
  attachments?: string[];
  attack_ready: boolean;
  characteristics?: Characteristics | null;
  controller: number;
  handle: string;
  keywords?: KeywordAbility[];
  marked_damage: ExactQuantity;
  oracle_id: string;
  owner: number;
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
  mana_value?: ExactQuantity | null;
  mode?: number | null;
  player_targets?: number[];
  spell: string | null;
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
  ability: number;
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
