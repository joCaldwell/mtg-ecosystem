/* Generated from Rust mtg-server schema; do not edit. */

export type ActionQueryRequest =
  | {
      operation: "start";
      page_size: number;
      scope: ActionScope;
      token: string;
      work: number;
    }
  | {
      cursor: string;
      operation: "continue";
      work: number;
    }
  | {
      cursor: string;
      operation: "cancel";
    };
export type ActionScope =
  | {
      scope: "window";
    }
  | {
      object: string;
      scope: "object";
    };
