/* Generated from Rust mtg-server schema; do not edit. */

/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ActivationCost".
 */
export type ActivationCost =
  | ("sacrifice_source" | "tap_source")
  | {
      discard_source: {
        mana: ManaCost;
      };
    }
  | {
      sacrifice_permanent: {
        count?: number;
        filter: SacrificeFilter;
        mana?: ManaCost | null;
      };
    }
  | {
      tap_pay_life_sacrifice_source: {
        life: ExactQuantity;
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
 * via the `definition` "CardKind".
 */
export type CardKind = "land" | "creature" | "instant" | "sorcery" | "enchantment" | "artifact";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "FunctioningZone".
 */
export type FunctioningZone = ("hand" | "battlefield" | "stack") | "any";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Instruction".
 */
export type Instruction = {
  node: Node;
  op: "kernel";
  source_block: number;
};
/**
 * Control over verbs. A node is performed in written order (CR 608.2c).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
      unless_pays: {
        node: Node;
        payer: PlayerExpr;
        payment: Payment;
      };
    }
  | {
      delayed: {
        event: DelayedEvent;
        node: Node;
      };
    }
  | {
      reflexive: {
        first: Node;
        then: Node;
      };
    };
/**
 * One game action. A verb acts on every member of its operands as one
 * simultaneous event unless its rule says otherwise (CR 608.2c, 101.4).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
      counter: CounterKind;
      on: SetExpr;
      verb: "put_counters";
    }
  | {
      controller: PlayerExpr;
      count: Amount;
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
      amount: "life_of";
      player: PlayerExpr;
    }
  | {
      amount: "other_casts_mana_value";
      caster: PlayerExpr;
    };
/**
 * The closed reference vocabulary. Every operand that is not a literal is
 * built from these, and the reference binder emits exactly these.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "EventPlayerRole".
 */
export type EventPlayerRole =
  ("damaged_player" | "controller_of_event_object") | "caster" | "activating_player" | "active_player";
/**
 * What an earlier step of the same node produced.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "StepRole".
 */
export type StepRole = "objects" | "player" | "amount";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * One conjunct of an object predicate. Card types, supertypes and subtypes
 * are the object's current ones (CR 205, 613.1d); colors are current colors
 * (CR 105, 613.1e); power, toughness and keywords are fully derived
 * (CR 613.4, 613.1f).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
      atom: "is_not";
      object: Ref;
    }
  | {
      alternatives: Pred[];
      atom: "any_of";
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Cmp".
 */
export type Cmp = "at_least" | "at_most" | "exactly";
/**
 * A conjunction of atoms. The empty predicate admits every object of the
 * zone it is evaluated in.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Pred".
 */
export type Pred = Atom[];
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Stat".
 */
export type Stat = "power" | "toughness" | "mana_value";
/**
 * Who or what damage is dealt to (CR 120.1-120.3).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "PredefinedToken".
 */
export type PredefinedToken = "treasure" | "food" | "clue";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Until".
 */
export type Until = "end_of_combat" | "end_of_turn" | "controller_next_turn" | "source_leaves" | "indefinite";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Change".
 */
export type Change =
  | {
      change: "power_toughness";
      power: Amount;
      toughness: Amount;
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
      change: "set_colors";
      colors: Color[];
    }
  | {
      change: "add_types";
      kinds: CardKind[];
    }
  | {
      change: "controller";
      player: PlayerExpr;
    };
/**
 * CR 613.1: the layer a continuous change applies in.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Layer".
 */
export type Layer =
  "copy" | "control" | "text" | "type" | "color" | "ability" | "power_toughness_set" | "power_toughness_modify";
/**
 * CR 603.7: when a delayed triggered ability triggers.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "DelayedEvent".
 */
export type DelayedEvent =
  | {
      at: "next_end_step";
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
 * Mandatory ordinary event patterns. ETB patterns inspect the event's
 * resulting battlefield, including the entering source itself (CR 603.6a).
 * Dies inspects the battlefield immediately before the actual graveyard move
 * (CR 603.10/700.4), including a source that dies simultaneously.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TriggerCondition".
 */
export type TriggerCondition =
  | (
      | "controlled_land_enters_battlefield"
      | "delayed_controller_creature_entry"
      | "delayed_controller_spell_cast"
      | "opponent_creature_dies"
      | "self_enters_battlefield"
      | "equipped_combat_damage"
      | "equipped_dies"
      | "self_dies"
      | "self_aura_to_graveyard"
    )
  | {
      saga_chapter: {
        number: number;
      };
    }
  | "self_spell_cast"
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
        spell:
          | TypePredicate
          | {
              color: "W" | "U" | "B" | "R" | "G";
              predicate: "spell_color";
            };
      };
    }
  | {
      spell_cast_with_origin: {
        caster: CasterRelation;
        origin: CastOriginPredicate;
        spell: TypePredicate;
      };
    }
  | {
      step_began: {
        step: BeginningStep;
        turn_relation: TurnRelation;
      };
    }
  | "self_becomes_tapped"
  | {
      mana_tapped: {
        filter: ManaTapFilter;
      };
    }
  | "other_creature_enters_battlefield"
  | {
      controlled_creature_enters_with_power: {
        minimum_power: ExactQuantity;
        other: boolean;
      };
    }
  | "any_creature_dies"
  | {
      self_moved: {
        from: SourceMoveOrigin;
        to: TriggerSuccessorZone;
      };
    };
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
 * Type membership includes additional types. Spell color uses frozen characteristics.
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
      predicate: "subtype";
      subtype: string;
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
 * CR 601.2a: compare the zone the spell left during announcement.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CastOriginPredicate".
 */
export type CastOriginPredicate = {
  predicate: "outside";
  zone: CastOriginZone;
};
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CastOriginZone".
 */
export type CastOriginZone = "controller_hand";
/**
 * Exact step predicates; phases, delayed and conditional boundaries are excluded.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "BeginningStep".
 */
export type BeginningStep = "precombat_main" | "upkeep" | "draw" | "end";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TurnRelation".
 */
export type TurnRelation = "controller" | "any_player" | "opponent";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ManaTapFilter".
 */
export type ManaTapFilter =
  | {
      filter: "land";
    }
  | {
      filter: "nonbasic_land";
    }
  | {
      filter: "land_subtype";
      subtype: BasicLandType;
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "BasicLandType".
 */
export type BasicLandType = "plains" | "island" | "swamp" | "mountain" | "forest";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SourceMoveOrigin".
 */
export type SourceMoveOrigin = "battlefield";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TriggerSuccessorZone".
 */
export type TriggerSuccessorZone = "trigger_controller_graveyard" | "trigger_controller_library";
/**
 * What a play permission allows (CR 601.3, 305.1).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Permission".
 */
export type Permission = "play" | "cast" | "cast_without_paying_mana_cost";
/**
 * A designation a player or object carries (CR 700.6, 725, 726).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Designation".
 */
export type Designation = "monarch" | "initiative" | "city_blessing" | "monstrous" | "renowned";
/**
 * A whole reviewed procedure the algebra does not decompose. Each id names
 * one Comprehensive Rules keyword action, never a card.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "BuiltinId".
 */
export type BuiltinId = "proliferate" | "cascade" | "demonstrate" | "hideaway";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
      cond: "exists";
      set: SetExpr;
    }
  | {
      cond: "cost_paid";
      identity: string;
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * CR 118: what a player may pay during resolution.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
    };
/**
 * Permanent resolution is distinct from an empty instruction program.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SpellResolution".
 */
export type SpellResolution =
  | ("one_or_more_modes" | "instructions" | "permanent")
  | {
      choose_modes: {
        exactly: number;
      };
    }
  | {
      choose_repeated_modes: {
        exactly: number;
      };
    };
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
  | "predefined_artifact_tokens_v1"
  | "player_graveyard_shuffle_v1"
  | "opening_placement_v1"
  | "controller_life_freeze_v1"
  | "protection_from_everything_v1"
  | "basic_landwalk_v1"
  | "fixed_color_protection_v1"
  | "source_combat_restrictions_v1"
  | "cast_history_v1"
  | "cast_mana_value_sweep_v1"
  | "cast_trigger_v1"
  | "cast_color_predicate_v1"
  | "cast_subtype_predicate_v1"
  | "mana_tap_trigger_v1"
  | "source_tap_trigger_v1"
  | "attack_trigger_v1"
  | "damage_trigger_v1"
  | "source_damage_trigger_v2"
  | "event_player_discard_v1"
  | "event_player_sacrifice_v1"
  | "filtered_damage_trigger_v1"
  | "damage_controller_discard_v1"
  | "event_player_recipient_v1"
  | "untap_controller_lands_v1"
  | "step_beginning_trigger_v1"
  | "enters_tapped_v1"
  | "typed_nonbasic_intrinsic_mana_v1"
  | "unspent_mana_conversion_v1"
  | "controller_land_granted_mana_v1"
  | "controller_land_basic_types_v1"
  | "controller_nonland_colors_v1"
  | "flash_v1"
  | "enter_as_copy_of_permanent_entered_this_turn_v1"
  | "cast_artifact_v1"
  | "permanent_mana_v1"
  | "composite_activation_cost_v1"
  | "cast_enchantment_v1"
  | "temporary_power_toughness_v1"
  | "combat_damage_prevention_v1"
  | "damage_budget_prevention_v1"
  | "static_damage_replacement_v1"
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
  | "power_filtered_removal_v1"
  | "untap_permanent_v1"
  | "tap_permanent_v1"
  | "activated_tap_permanent_v1"
  | "activated_creature_tap_v1"
  | "tap_source_destroy_v1"
  | "tap_source_untap_creature_v1"
  | "activated_permanent_owner_hand_v1"
  | "activated_artifact_draw_v1"
  | "owner_hand_move_v1"
  | "permanent_owner_hand_move_v1"
  | "cast_sorcery_v1"
  | "owner_library_top_move_v1"
  | "activated_ability_v1"
  | "sacrifice_source_cost_v1"
  | "controller_draw_v1"
  | "fixed_mill_program_v1"
  | "controller_counted_draw_v1"
  | "controller_discard_v1"
  | "opponent_target_v1"
  | "target_player_sacrifice_creature_v1"
  | "stack_spell_target_v1"
  | "stack_ability_target_v1"
  | "counter_stack_object_v1"
  | "controlled_creature_spell_copy_v1"
  | "tap_source_cost_v1"
  | "pay_life_cost_v1"
  | "mana_activation_cost_v1"
  | "discard_source_cost_v1"
  | "any_damage_target_v1"
  | "player_or_planeswalker_target_v1"
  | "any_player_target_v1"
  | "noncombat_damage_v1"
  | "life_gain_v1"
  | "life_loss_v1"
  | "opponent_life_drain_v1"
  | "kernel_control_v1"
  | "triggered_ability_v1"
  | "self_enters_battlefield_v1"
  | "other_creature_enters_battlefield_v1"
  | "entry_graveyard_exile_v1"
  | "controlled_land_entry_v1"
  | "land_reclamation_v1"
  | "creature_dies_v1"
  | "self_dies_v1"
  | "intervening_if_v1"
  | "creature_or_planeswalker_target_v1"
  | "modal_choice_v1"
  | "defender_v1"
  | "flying_v1"
  | "reach_v1"
  | "vigilance_v1"
  | "haste_v1"
  | "lifelink_v1"
  | "deathtouch_v1"
  | "indestructible_v1"
  | "activated_source_keyword_grant_v1"
  | "trample_v1"
  | "first_strike_v1"
  | "double_strike_v1"
  | "menace_v1"
  | "creature_tokens_v1"
  | "multi_type_creature_tokens_v1"
  | "plus_one_counters_v1"
  | "destroy_creature_v1"
  | "graveyard_recursion_v1"
  | "own_graveyard_card_return_v1"
  | "basic_land_search_v1"
  | "creature_anthem_v1"
  | "target_player_draw_v1"
  | "attachments_v1"
  | "permanent_removal_v1"
  | "sacrifice_v1"
  | "fixed_artifact_sacrifice_cost_v1"
  | "board_wipes_v1"
  | "self_spell_protection_v1"
  | "announced_x_draw_v1"
  | "each_player_draw_v1"
  | "announced_x_effects_v1"
  | "target_player_discard_v1"
  | "random_player_discard_v1"
  | "group_hand_discard_v1"
  | "composed_ability_v1";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "HybridPayment".
 */
export type HybridPayment = "pay_two_life" | Color;
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ManaProduction".
 */
export type ManaProduction =
  | {
      mana: "restricted_fixed";
      spend: ManaSpendRestriction;
      symbols: Color[];
    }
  | {
      life: ExactQuantity;
      mana: "fixed_with_life_gain";
      symbols: Color[];
    }
  | {
      /**
       * @minItems 2
       * @maxItems 2
       */
      colors: [Color, Color];
      mana: "hybrid_filter";
    }
  | {
      mana: "greatest_controlled_power_combination";
    }
  | {
      mana: "fixed";
      symbols: Color[];
    }
  | {
      mana: "any_color";
    }
  | {
      amount: SingleColorManaAmount;
      mana: "any_one_color";
    }
  | {
      mana: "commander_color";
    }
  | {
      mana: "opponent_land_colors";
    }
  | {
      mana: "each_permanent_color";
      scope: PermanentColorScope;
    }
  | {
      colors: Color[];
      mana: "choice";
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ManaSpellSubtype".
 */
export type ManaSpellSubtype = "dwarf" | "equipment" | "saga";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SingleColorManaAmount".
 */
export type SingleColorManaAmount =
  | {
      kind: "fixed";
      quantity: ExactQuantity;
    }
  | {
      kind: "controller_enchantments";
    };
/**
 * CR 109.5/110.1: battlefield permanents controlled by the static source's
 * current controller or by the player who activated the mana ability.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "PermanentColorScope".
 */
export type PermanentColorScope = "controller";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ManaActivationRestriction".
 */
export type ManaActivationRestriction = "once_each_turn";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "StaticEffect".
 */
export type StaticEffect =
  | (
      | "additional_land_play"
      | "play_lands_from_own_graveyard"
      | "keep_unspent_mana_as_black"
      | "grant_controller_lands_any_color_mana"
      | "may_begin_on_battlefield"
      | "grant_controller_lands_all_basic_types"
      | "controller_nonlands_all_colors"
      | "enters_tapped"
      | "reduce_generic_by_creature_count"
      | "double_damage_to_permanent_or_player"
    )
  | {
      bestow: {
        cost: ManaCost;
        program: SpellProgram;
      };
    }
  | {
      grant_spell_cascade: {
        eligibility: SpellCascadeEligibility;
        program: SpellProgram;
      };
    }
  | "cascade_land_entry"
  | "enter_as_copy_of_permanent_entered_this_turn"
  | "control_enchanted_permanent"
  | "opponents_cant_cast_during_controller_turn"
  | "unlimited_controller_hand_size"
  | {
      reduce_maximum_hand_size: {
        amount: string;
        scope: HandSizeScope;
      };
    }
  | {
      increase_maximum_hand_size: {
        amount: string;
      };
    }
  | {
      enters_tapped_unless_controller_basic_lands: {
        minimum: ExactQuantity;
      };
    }
  | {
      enters_tapped_unless_controller_other_lands: {
        comparison: LandCountComparison;
        threshold: string;
      };
    }
  | {
      enters_tapped_unless_opponent_count: {
        minimum: ExactQuantity;
      };
    }
  | {
      enters_tapped_unless_reveal_land_types: {
        /**
         * @minItems 2
         * @maxItems 2
         */
        subtypes: [BasicLandType, BasicLandType];
      };
    }
  | {
      enters_tapped_unless_controller_land_type: {
        subtype: BasicLandType;
      };
    }
  | {
      enters_tapped_unless_controller_land_types: {
        /**
         * @minItems 2
         * @maxItems 2
         */
        subtypes: [BasicLandType, BasicLandType];
      };
    }
  | {
      enters_tapped_unless_pay_life: {
        amount: ExactQuantity;
      };
    }
  | {
      reduce_self_generic_cost: {
        count: BattlefieldCount;
        per_object: ExactQuantity;
      };
    }
  | "cant_be_countered"
  | "self_graveyard_reveal_shuffle"
  | "grant_other_controlled_permanents_indestructible"
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
      modify_source_by_distinct_permanent_colors: {
        power_per_color: ExactQuantity;
        scope: PermanentColorScope;
        toughness_per_color: ExactQuantity;
      };
    }
  | {
      controller_creature_anthem: {
        power: ExactQuantity;
        toughness: ExactQuantity;
      };
    }
  | {
      controller_scoped_creature_stats: {
        filter: ControllerCreatureStatFilter;
        other: boolean;
        power: ExactQuantity;
        toughness: ExactQuantity;
      };
    }
  | {
      controller_creature_plus_one_counter_replacement: {
        modification: CounterPlacementModification;
      };
    }
  | "double_controller_creature_damage_to_permanent_or_player";
/**
 * Disjoint predicates on a completed spell; history is global to its caster.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SpellCascadeEligibility".
 */
export type SpellCascadeEligibility =
  | {
      eligibility: "mana_value_at_least";
      minimum: ExactQuantity;
    }
  | {
      eligibility: "first_cast_each_turn";
    }
  | {
      eligibility: "first_cast_from_exile_each_turn";
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "HandSizeScope".
 */
export type HandSizeScope = "controller" | "opponents";
/**
 * Inclusive comparison for a common-prior land count (CR 614.12).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "LandCountComparison".
 */
export type LandCountComparison = "at_least" | "at_most";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CountController".
 */
export type CountController = "any" | "caster";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CreatureRestriction".
 */
export type CreatureRestriction = "cant_block" | "cant_be_blocked" | "cant_attack_or_block" | "can_block_only_flying";
/**
 * Static scopes in the finite attachment pool (CR 611.3a, 613.1f/4c).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CreatureScope".
 */
export type CreatureScope =
  | ("equipped" | "equipped_legendary" | "enchanted" | "controller" | "source")
  | "attacking_controller"
  | {
      controlled_subtype: {
        creatures_only: boolean;
        other: boolean;
        subtype: Subtype;
      };
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Subtype".
 */
export type Subtype = "elf" | "goblin" | "golem" | "merfolk" | "sliver" | "soldier" | "vampire" | "warrior" | "zombie";
/**
 * Restrictions evaluated on the affected creature's earlier-layer characteristics.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ControllerCreatureStatFilter".
 */
export type ControllerCreatureStatFilter =
  | {
      selector: "any";
    }
  | {
      selector: "artifact";
    }
  | {
      selector: "subtypes";
      subtypes: string[];
    };
/**
 * CR 614.1a/614.5: positive placement quantities only; removal is a cost.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "InterveningCondition".
 */
export type InterveningCondition =
  | ("controller_hand_empty" | "source_cast_by_controller")
  | {
      source_status: {
        status: TapStatus;
      };
    }
  | {
      controller_has_creature_with_power: {
        minimum_power: ExactQuantity;
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
  /**
   * True only when `composition` admitted every ability of this definition.
   */
  composed?: boolean;
  intrinsic_zone: FunctioningZone | null;
  keywords?: KeywordAbility[];
  mana_abilities?: ManaAbility[];
  modal_land?: ModalLand | null;
  name: string;
  oracle_id: string;
  provenance: SourceProvenance;
  /**
   * Offline compiler-owned labels; absent on historical IR and synthetic definitions.
   */
  source_labels?: SourceLabels | null;
  spell: SpellProgram | null;
  static_abilities?: StaticAbility[];
  transform_faces?: TransformFaces | null;
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
 * via the `definition` "ManaCost".
 */
export interface ManaCost {
  colored: Color[];
  generic: ExactQuantity;
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
 * Finite permanent costs; costs are committed only after complete payment.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "PermanentActivationCosts".
 */
export interface PermanentActivationCosts {
  mana?: ManaCost | null;
  remove_source_plus_one_counters?: ExactQuantity | null;
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
  optional_additional_costs?: OptionalAdditionalManaCost[];
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
  additional_types?: CardKind[];
  colors: Color[];
  keywords: KeywordAbility[];
  power: ExactQuantity;
  subtypes: string[];
  toughness: ExactQuantity;
}
/**
 * CR 701.23: what a search looks for and where the found cards go.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SearchSpec".
 */
export interface SearchSpec {
  filter: Pred;
  maximum: Amount;
  reveal?: boolean;
  to: Destination;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SpellMode".
 */
export interface SpellMode {
  program: SpellProgram;
}
/**
 * CR 702.33a/e: one source-bound optional fixed mana declaration. The
 * identity names the exact archived keyword; conditions cannot link by position.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "OptionalAdditionalManaCost".
 */
export interface OptionalAdditionalManaCost {
  identity: string;
  mana: ManaCost;
  source_block: number;
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ManaAbility".
 */
export interface ManaAbility {
  costs: PermanentActivationCosts;
  production: ManaProduction;
  restriction?: ManaActivationRestriction | null;
  /**
   * Damage dealt by the source to the activating player after producing mana.
   */
  self_damage?: ExactQuantity | null;
  source_block: number;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ModalLand".
 */
export interface ModalLand {
  back: LandFace;
  front_name: string;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "LandFace".
 */
export interface LandFace {
  color: Color;
  name: string;
  tapped: boolean;
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
 * Count current battlefield types and control, never ownership (CR 702.41).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "BattlefieldCount".
 */
export interface BattlefieldCount {
  controller: CountController;
  kind: CardKind;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TransformFaces".
 */
export interface TransformFaces {
  back: CreatureFace;
  front_name: string;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CreatureFace".
 */
export interface CreatureFace {
  name: string;
  power: ExactQuantity;
  subtypes: string[];
  toughness: ExactQuantity;
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
