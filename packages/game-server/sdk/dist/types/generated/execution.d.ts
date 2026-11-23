/* Generated from Rust mtg-server schema; do not edit. */

/**
 * This interface was referenced by `CommandExecution`'s JSON-Schema
 * via the `definition` "CommandExecutionStatus".
 */
export type CommandExecutionStatus =
  | {
      status: "pending";
    }
  | {
      status: "consumed_by_rules";
    }
  | {
      status: "discarded_by_departure";
    }
  | {
      status: "cancelled";
    }
  | {
      reason: RejectionReason;
      status: "rejected";
    };
/**
 * This interface was referenced by `CommandExecution`'s JSON-Schema
 * via the `definition` "RejectionReason".
 */
export type RejectionReason =
  "stale_revision" | "invalid_response" | "already_finished" | "resource_paused" | "cancelled";

export interface CommandExecution {
  command_id: string;
  status: CommandExecutionStatus;
}
