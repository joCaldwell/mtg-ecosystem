/* Generated from Rust mtg-server schema; do not edit. */

/**
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "ActionQueryStatus".
 */
export type ActionQueryStatus =
  | {
      page: ActionPage;
      status: "complete";
    }
  | {
      cursor: string;
      page: ActionPage;
      required_minimum: number;
      status: "incomplete";
    }
  | {
      status: "stale";
    }
  | {
      reason: ActionQueryUnavailable;
      status: "unavailable";
    }
  | {
      status: "cancelled";
    };
/**
 * Finite beginnings; deliberately excludes target/payment/X combinations.
 *
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "ActionInitiation".
 */
export type ActionInitiation =
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
    }
  | {
      action: "finish_mana";
      undo_mana: boolean;
    };
/**
 * Core action wording with optional exact compiler-bound source description.
 *
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "ActionLabel".
 */
export type ActionLabel =
  | "unavailable"
  | {
      available: {
        source?: DecisionSource | null;
        text: string;
      };
    };
/**
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "DecisionSourceKind".
 */
export type DecisionSourceKind = "spell" | "activated" | "triggered" | "mana_ability" | "effect";
/**
 * Minimal context copied from the permitted request, with no inferred labels.
 *
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "ActionDecisionContext".
 */
export type ActionDecisionContext =
  | {
      window: "priority";
    }
  | {
      cost: ManaCost;
      source: ActionSourceContext;
      window: "mana";
    };
/**
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "Color".
 */
export type Color = "W" | "U" | "B" | "R" | "G" | "C";
/**
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "ExactQuantity".
 */
export type ExactQuantity = string;
/**
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "ActionSourceContext".
 */
export type ActionSourceContext =
  | {
      object: string;
      status: "available";
    }
  | {
      status: "unavailable";
    };
/**
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "ActionScope".
 */
export type ActionScope =
  | {
      scope: "window";
    }
  | {
      object: string;
      scope: "object";
    };
/**
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "ActionQueryUnavailable".
 */
export type ActionQueryUnavailable =
  | {
      kind: "core";
      reason: ActionUnavailable;
    }
  | {
      kind: "quota";
    }
  | {
      kind: "in_flight";
    }
  | {
      kind: "total_work";
    }
  | {
      kind: "response_limit";
    }
  | {
      kind: "executor";
    };
/**
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "ActionUnavailable".
 */
export type ActionUnavailable =
  "invalid_viewer" | "invalid_page_size" | "boundary" | "unsupported_window" | "scope" | "invariant";

export interface ActionQueryResponse {
  match_id: string;
  outcome: ActionQueryStatus;
  protocol_version: number;
  viewer: number;
}
/**
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "ActionPage".
 */
export interface ActionPage {
  /**
   * Incremental page; resume never repeats earlier emitted entries.
   *
   * @maxItems 32
   */
  beginnings: ActionBeginning[];
  /**
   * None only while initialization has not received a budget unit.
   */
  context: ActionDecisionContext | null;
  scope: ActionScope;
  token: string;
}
/**
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "ActionBeginning".
 */
export interface ActionBeginning {
  initiation: ActionInitiation;
  label: ActionLabel;
}
/**
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
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
 * This interface was referenced by `ActionQueryResponse`'s JSON-Schema
 * via the `definition` "ManaCost".
 */
export interface ManaCost {
  colored: Color[];
  generic: ExactQuantity;
}
