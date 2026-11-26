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
      exile_source: {
        mana: ManaCost;
      };
    }
  | {
      choose_objects: ChosenObjectCost;
    }
  | {
      sacrifice_permanent: {
        /**
         * CR 109.1, 701.21a: "another" excludes the ability's own source.
         */
        another?: boolean;
        count?: number;
        filter: SacrificeFilter;
        mana?: ManaCost | null;
        /**
         * CR 602.2b, 701.21a: "{T}" is paid with the sacrifice, after mana.
         */
        tap?: boolean;
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
 * via the `definition` "ChosenCostAction".
 */
export type ChosenCostAction = "discard" | "tap" | "exile_from_graveyard";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ChosenCostFilter".
 */
export type ChosenCostFilter =
  | ("any" | "artifact_or_creature")
  | {
      type: CardKind;
    }
  | {
      subtype: string;
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CardKind".
 */
export type CardKind = "land" | "creature" | "instant" | "sorcery" | "enchantment" | "artifact";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "FunctioningZone".
 */
export type FunctioningZone = ("hand" | "graveyard" | "battlefield" | "stack") | "any";
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
      affects: CombatProhibitionScope;
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
  | ("damaged_player" | "controller_of_event_object")
  | "caster"
  | "activating_player"
  | "drawing_player"
  | "defending_player"
  | "active_player";
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
  | "protection_from_everything"
  | "intimidate"
  | "shadow"
  | "horsemanship"
  | "fear"
  | "skulk";
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
 * Player counters for poison and energy (CR 107.14, 122.1f, 704.5c).
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "PlayerCounterKind".
 */
export type PlayerCounterKind = "poison" | "energy";
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
export type PredefinedToken = "treasure" | "gold" | "food" | "clue";
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ManaLandController".
 */
export type ManaLandController = "you" | "opponents";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ManaActivationRestriction".
 */
export type ManaActivationRestriction = "once_each_turn";
/**
 * CR 613.1: the layer a continuous change applies in.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Layer".
 */
export type Layer =
  "copy" | "control" | "text" | "type" | "color" | "ability" | "power_toughness_set" | "power_toughness_modify";
/**
 * CR 611.2c: a rules restriction can name one exact object or apply to a
 * live class of creatures. It does not grant or remove an ability.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "CombatProhibitionScope".
 */
export type CombatProhibitionScope =
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "BeginningStep".
 */
export type BeginningStep =
  "precombat_main" | "upkeep" | "draw" | "end" | "beginning_of_combat" | "end_of_combat" | "postcombat_main";
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ObjectChoiceAction".
 */
export type ObjectChoiceAction = "return_to_owner_hand";
/**
 * The simultaneous battlefield removal a counted target group performs.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "GroupRemoval".
 */
export type GroupRemoval = "destroy" | "exile" | "owner_hand";
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
 * via the `definition` "ActivationWindow".
 */
export type ActivationWindow =
  ("your_turn" | "your_upkeep" | "your_turn_before_attackers") | "combat" | "end_of_combat" | "not_your_turn";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Capability".
 */
export type Capability =
  | (
      | "predefined_artifact_tokens_v1"
      | "player_graveyard_shuffle_v1"
      | "controller_life_freeze_v1"
      | "protection_from_everything_v1"
      | "basic_landwalk_v1"
      | "evasion_keywords_v1"
      | "fixed_color_protection_v1"
      | "source_combat_restrictions_v1"
      | "cast_history_v1"
      | "cast_mana_value_sweep_v1"
      | "cast_trigger_v1"
      | "cast_color_predicate_v1"
      | "cast_subtype_predicate_v1"
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
      | "entry_life_payment_v1"
      | "entry_reveal_v1"
      | "conditional_entry_tapped_v1"
      | "entry_plus_one_counters_v1"
      | "typed_nonbasic_intrinsic_mana_v1"
      | "flash_v1"
      | "enter_as_copy_of_permanent_entered_this_turn_v1"
      | "cast_artifact_v1"
      | "permanent_mana_v1"
      | "composite_activation_cost_v1"
      | "cast_enchantment_v1"
      | "temporary_power_toughness_v1"
      | "temporary_combat_prohibition_v1"
      | "resolved_control_v1"
      | "resolved_characteristics_v1"
      | "combat_damage_prevention_v1"
      | "damage_budget_prevention_v1"
      | "static_damage_replacement_v1"
      | "normal_layout_v1"
      | "modal_double_faced_v1"
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
      | "optional_object_target_v1"
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
      | "regeneration_shield_v1"
      | "controlled_creature_spell_copy_v1"
      | "tap_source_cost_v1"
      | "pay_life_cost_v1"
      | "mana_activation_cost_v1"
      | "discard_source_cost_v1"
      | "exile_source_cost_v1"
      | "chosen_object_cost_v1"
      | "any_damage_target_v1"
      | "player_or_planeswalker_target_v1"
      | "any_player_target_v1"
      | "noncombat_damage_v1"
      | "life_gain_v1"
      | "life_loss_v1"
      | "opponent_life_drain_v1"
      | "kernel_control_v1"
      | "resolution_object_choice_v1"
      | "target_characteristics_v1"
      | "optional_target_group_v1"
      | "triggered_ability_v1"
      | "self_enters_battlefield_v1"
      | "other_creature_enters_battlefield_v1"
      | "entry_graveyard_exile_v1"
      | "controlled_land_entry_v1"
      | "creature_dies_v1"
      | "self_dies_v1"
      | "intervening_if_v1"
      | "resolution_trigger_v1"
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
      | "fight_v1"
      | "populate_v1"
      | "player_counters_v1"
      | "proliferate_v1"
      | "creature_tokens_v1"
      | "multi_type_creature_tokens_v1"
      | "plus_one_counters_v1"
      | "destroy_creature_v1"
      | "graveyard_recursion_v1"
      | "own_graveyard_card_return_v1"
      | "private_library_reorder_v1"
      | "search_library_top_v1"
      | "basic_land_search_v1"
      | "creature_anthem_v1"
      | "continuous_statics_v1"
      | "rule_statics_v1"
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
      | "composed_ability_v1"
    )
  | "transforming_double_faced_v1"
  | "adventurer_v1"
  | "transform_source_v1"
  | "counted_target_group_v1"
  | "target_group_removal_v1"
  | "event_trigger_v1"
  | "trigger_turn_history_v1"
  | "granted_mana_abilities_v1";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "HybridPayment".
 */
export type HybridPayment = "pay_two_life" | Color;
/**
 * How the two printed parts of one physical card relate (CR 712, 715).
 * <https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "FaceLayout".
 */
export type FaceLayout = "single" | "modal_double_faced" | "transforming" | "adventurer";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "StaticEffect".
 */
export type StaticEffect =
  | "enters_tapped"
  | {
      pay_life_or_enter_tapped: {
        amount: ExactQuantity;
      };
    }
  | {
      reveal_or_enter_tapped: {
        filter: Pred;
      };
    }
  | {
      enters_tapped_if: {
        condition: EntryCondition;
      };
    }
  | {
      enters_with_plus_one_counters: {
        count: ExactQuantity;
      };
    }
  | {
      continuous: {
        affects: Scope;
        changes: Change[];
        condition?: Cond | null;
      };
    }
  | {
      rule: {
        condition?: Cond | null;
        rule: StaticRule;
      };
    };
/**
 * CR 614.12: evaluated against the pre-entry battlefield for the incoming controller.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "EntryCondition".
 */
export type EntryCondition =
  | {
      unless_controlled: {
        cmp: Cmp;
        count: ExactQuantity;
        filter: Pred;
      };
    }
  | {
      unless_opponents: {
        cmp: Cmp;
        count: ExactQuantity;
      };
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Scope".
 */
export type Scope =
  | {
      scope: "source";
    }
  | {
      filter: Pred;
      scope: "matching";
    }
  | {
      filter: Pred;
      kind: AttachmentKind;
      scope: "attachment";
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "AttachmentKind".
 */
export type AttachmentKind = "equipped" | "enchanted";
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "StaticRule".
 */
export type StaticRule =
  | {
      affects: Scope;
      requirement: CombatRequirement;
      rule: "combat";
    }
  | {
      affects: Scope;
      rule: "cant_be_countered";
    }
  | {
      affects: Scope;
      caster: PlayerExpr;
      change: SpellCostChange;
      multiplier?: Amount | null;
      rule: "spell_cost";
    }
  | {
      change: HandSizeChange;
      player: PlayerExpr;
      rule: "maximum_hand_size";
    }
  | {
      allowance: Allowance;
      player: PlayerExpr;
      rule: "additional_land_plays";
    }
  | {
      affects: Scope;
      caster: PlayerExpr;
      rule: "cast_as_though_flash";
    }
  | {
      affects: Scope;
      restriction: BlockRestriction;
      rule: "block";
    }
  | {
      affects: Scope;
      defender: PlayerExpr;
      rule: "attack_restriction";
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "SpellCostChange".
 */
export type SpellCostChange =
  | {
      increase: {
        mana: ManaCost;
      };
    }
  | {
      reduce: {
        mana: ManaCost;
      };
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "HandSizeChange".
 */
export type HandSizeChange =
  | "unlimited"
  | {
      set: {
        size: ExactQuantity;
      };
    }
  | {
      adjust: {
        delta: ExactQuantity;
      };
    };
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Allowance".
 */
export type Allowance =
  | "unlimited"
  | {
      additional: {
        count: ExactQuantity;
      };
    };
/**
 * CR 509.1b: per-pair and per-attacker block restrictions.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "BlockRestriction".
 */
export type BlockRestriction =
  | {
      filter: Pred;
      only: boolean;
      restriction: "blockers";
    }
  | {
      filter: Pred;
      only: boolean;
      restriction: "attackers";
    }
  | {
      count: number;
      restriction: "at_most";
    }
  | {
      count: number;
      restriction: "at_least";
    };

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
  /**
   * The other printed part of a two-part card: a double-faced card's back
   * face or an adventurer's Adventure. Only a physical card's root
   * definition owns it.
   */
  back_face?: Definition | null;
  capabilities: Capability[];
  characteristics: Characteristics;
  /**
   * True only when `composition` admitted every ability of this definition.
   */
  composed?: boolean;
  /**
   * CR 702.34a: "Flashback [cost]": this instant or sorcery may be cast
   * from its owner's graveyard for this mana instead of its mana cost,
   * and is exiled whenever it would leave the stack.
   */
  flashback?: ManaCost | null;
  intrinsic_zone: FunctioningZone | null;
  keywords?: KeywordAbility[];
  /**
   * How `back_face` relates to this front face.
   */
  layout?: FaceLayout;
  mana_abilities?: ManaAbility[];
  modal_spell?: ModalSpell | null;
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
  /**
   * CR 602.2b, 700.2: a modal ability chooses its modes as it is
   * activated; `program` then carries timing only.
   */
  modal?: ModalSpell | null;
  program: SpellProgram;
  restriction?: ActivationRestriction | null;
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
 * CR 602.2b, 601.2g-h: one fixed chosen-object term paid after any mana.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ChosenObjectCost".
 */
export interface ChosenObjectCost {
  action: ChosenCostAction;
  another: boolean;
  count: number;
  filter: ChosenCostFilter;
  mana: ManaCost | null;
  tap_source: boolean;
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
 * CR 601.2b, 602.2b, 603.3c and 700.2a–d: one announcement or placement
 * chooses distinct modes. Each admitted selection is an ordinary program in
 * printed order with only its own target occurrences. Bit zero denotes the
 * first printed mode. Spells, activated and triggered abilities share it.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ModalSpell".
 */
export interface ModalSpell {
  labels: string[];
  max: number;
  min: number;
  selections: {
    [k: string]: SpellProgram;
  };
  source_block: number;
}
/**
 * This interface was referenced by `undefined`'s JSON-Schema definition
 * via the `patternProperty` "^\d+$".
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * CR 111.3–4: precisely defined creature tokens, never copied card definitions.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * Fixed values installed by one resolving effect (CR 611.2c, 613.6).
 * An omitted characteristic retains its value from the other effects.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "Animation".
 */
export interface Animation {
  add_artifact: boolean;
  creature_subtypes: string[];
  replace_creature_subtypes: boolean;
}
/**
 * This interface was referenced by `PublicManifest`'s JSON-Schema
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
 * CR 603.2: "your/their Nth [filtered] spell/card each turn", or with
 * `any_player` "the Nth [filtered] spell of a turn": the event triggers only
 * when it is the `nth` matching event of the current turn, counted from the
 * turn's start whether or not the source was on the battlefield then.
 * Events of the event's player count, or of every player with `any_player`.
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "TurnOrdinal".
 */
export interface TurnOrdinal {
  any_player?: boolean;
  nth: number;
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
 * CR 602.5b-d (2026-09-25): restrictions on an individual printed stack
 * activation. Sorcery timing uses the program's existing timing field.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
 *
 * This interface was referenced by `PublicManifest`'s JSON-Schema
 * via the `definition` "ActivationRestriction".
 */
export interface ActivationRestriction {
  /**
   * CR 602.1b, 602.5: "Activate only if"; one
   * [`kir::ActivationCondition`] read as the activation begins.
   */
  condition?: Cond | null;
  once_each_turn?: boolean;
  window?: ActivationWindow | null;
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
  functioning_zone: FunctioningZone;
  /**
   * CR 603.3c, 700.2: a modal trigger's modes are chosen as it is put on
   * the stack; `program` then carries timing only.
   */
  modal?: ModalSpell | null;
  /**
   * This mandatory stack program is placed by the rules, never cast or
   * activated. Its existing Instant shape carries no timing permission.
   */
  program: SpellProgram;
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
