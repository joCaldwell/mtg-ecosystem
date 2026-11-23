/* Generated from Rust mtg-server schema; do not edit. */

/**
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "CommandBody".
 */
export type CommandBody =
  | {
      kind: "decision";
      response: PlayerResponse;
      token: string;
    }
  | {
      kind: "concede";
    };
/**
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "PlayerResponse".
 */
export type PlayerResponse =
  | {
      keep: string;
      response: "legend_rule";
    }
  | {
      response: "announce_x";
      value: ExactQuantity;
    }
  | {
      object: string;
      response: "sacrifice_cost";
    }
  | {
      object: string;
      response: "forced_sacrifice";
    }
  | {
      color: Color;
      response: "mana_color";
    }
  | {
      objects: string[];
      response: "search_lands";
    }
  | {
      perform: boolean;
      response: "optional_effect";
    }
  | {
      perform: boolean;
      response: "optional_recursion";
    }
  | {
      response: "mulligan";
      take: boolean;
    }
  | {
      objects: string[];
      response: "bottom_opening";
    }
  | {
      allocations: [string, ExactQuantity][];
      response: "damage_allocation";
    }
  | {
      mode: number;
      response: "modes";
    }
  | {
      damage: string;
      effect: string;
      response: "damage_effect";
    }
  | {
      order: string[];
      response: "order_triggers";
    }
  | {
      entities: PlayerTargetEntity[];
      response: "entity_targets";
    }
  | {
      entries: string[];
      response: "stack_targets";
    }
  | {
      ability: number;
      object: string;
      response: "activate_ability";
    }
  | {
      players: number[];
      response: "player_targets";
    }
  | {
      object: string;
      response: "sacrifice";
    }
  | {
      objects: string[];
      response: "resolve_discard";
    }
  | {
      response: "spend_mana";
      spending: {
        B?: ExactQuantity;
        C?: ExactQuantity;
        G?: ExactQuantity;
        R?: ExactQuantity;
        U?: ExactQuantity;
        W?: ExactQuantity;
      };
    }
  | {
      response: "commander_return";
      to_command: boolean;
    }
  | {
      attacks: [string, number][];
      response: "attackers";
    }
  | {
      blocks: [string, string][];
      response: "blockers";
    }
  | {
      assignments: [string, ExactQuantity][];
      response: "damage";
    }
  | {
      amount: ExactQuantity;
      assignments: [string, ExactQuantity][];
      player: number;
      response: "trample_damage";
    }
  | {
      objects: string[];
      response: "discard";
    }
  | {
      object: string;
      response: "play_land";
    }
  | {
      object: string;
      response: "activate_mana";
    }
  | {
      card: string;
      response: "cast";
    }
  | {
      response: "pass";
    }
  | {
      objects: string[];
      response: "targets";
    }
  | {
      response: "finish_mana";
      undo_mana: boolean;
    };
/**
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "ExactQuantity".
 */
export type ExactQuantity = string;
/**
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "Color".
 */
export type Color = "W" | "U" | "B" | "R" | "G" | "C";
/**
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
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
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "DecimalU64Counter".
 */
export type DecimalU64Counter = string;

export interface CommandEnvelope {
  command: CommandBody;
  command_id: string;
  expected_revision: DecimalU64Counter;
  protocol_version: number;
}
