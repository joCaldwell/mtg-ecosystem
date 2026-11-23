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
      object?: string | null;
      response: "opening_placement";
    }
  | {
      response: "additional_costs";
      selected: string[];
    }
  | {
      object?: string | null;
      response: "entry_copy";
    }
  | {
      object: string;
      response: "entry_attachment";
    }
  | {
      keep: string;
      response: "legend_rule";
    }
  | {
      colors: HybridPayment[];
      response: "hybrid_cost";
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
      ability: ManaAbilityIdentity;
      response: "choose_mana_ability";
    }
  | {
      /**
       * @minItems 5
       * @maxItems 5
       */
      amounts: [ExactQuantity, ExactQuantity, ExactQuantity, ExactQuantity, ExactQuantity];
      response: "mana_combination";
    }
  | {
      color: Color;
      response: "mana_color";
    }
  | {
      card?: string | null;
      response: "entry_reveal";
    }
  | {
      perform: boolean;
      response: "optional_effect";
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
      modes: number[];
      response: "mode_subset";
    }
  | {
      mode: number;
      response: "modes";
    }
  | {
      effect: CounterPlacementEffectHandle;
      response: "counter_placement_effect";
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
      response: "spend_mana_allocated";
      restricted: RestrictedMana[];
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
      response: "play_back_land";
    }
  | {
      object: string;
      response: "activate_mana";
    }
  | {
      card: string;
      response: "bestow";
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
 * via the `definition` "HybridPayment".
 */
export type HybridPayment = "pay_two_life" | Color;
/**
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "Color".
 */
export type Color = "W" | "U" | "B" | "R" | "G" | "C";
/**
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "ExactQuantity".
 */
export type ExactQuantity = string;
/**
 * The intrinsic ability for a basic land type or an index into this source's printed mana abilities.
 *
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
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
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "ManaSpellSubtype".
 */
export type ManaSpellSubtype = "dwarf" | "equipment" | "saga";
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
/**
 * A viewer's public provider reference plus the definition's ability index.
 * Decision tokens bind it to one exact instruction and applied prefix.
 *
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "CounterPlacementEffectHandle".
 */
export interface CounterPlacementEffectHandle {
  ability: number;
  source: string;
}
/**
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "RestrictedMana".
 */
export interface RestrictedMana {
  amount: ExactQuantity;
  color: Color;
  restriction: ManaSpendRestriction;
}
