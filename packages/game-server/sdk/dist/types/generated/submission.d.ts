/* Generated from Rust mtg-server schema; do not edit. */

export type SubmissionResult =
  | {
      command_id: string;
      status: "accepted";
    }
  | {
      command_id: string;
      reason: RejectionReason;
      status: "rejected";
    };
export type RejectionReason =
  "stale_revision" | "invalid_response" | "already_finished" | "resource_paused" | "cancelled";
