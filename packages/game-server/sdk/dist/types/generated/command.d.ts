/* Generated from Rust mtg-server schema; do not edit. */

/**
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "CommandBody".
 */
export type CommandBody =
  | {
      answer: Answer;
      kind: "decision";
      token: string;
    }
  | {
      kind: "concede";
    };
/**
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "Answer".
 */
export type Answer =
  | {
      answer: "pick";
      picks: number[][];
    }
  | {
      answer: "order";
      order: number[];
    }
  | {
      amounts: ExactQuantity[];
      answer: "distribute";
    }
  | {
      answer: "number";
      value: ExactQuantity;
    }
  | {
      answer: "pay";
      pay: PayAnswer;
    };
/**
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "ExactQuantity".
 */
export type ExactQuantity = string;
/**
 * This interface was referenced by `CommandEnvelope`'s JSON-Schema
 * via the `definition` "PayAnswer".
 */
export type PayAnswer =
  | {
      source: number;
      step: "activate";
    }
  | {
      step: "finish";
      undo_on_failure: boolean;
    }
  | {
      step: "spend";
      unrestricted: {
        B?: ExactQuantity;
        C?: ExactQuantity;
        G?: ExactQuantity;
        R?: ExactQuantity;
        U?: ExactQuantity;
        W?: ExactQuantity;
      };
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
