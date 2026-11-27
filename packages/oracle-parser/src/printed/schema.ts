// Wire coordinates are half-open UTF-8 byte ranges with u32 endpoints.
export type ByteRange = [start: number, end: number];

export type Edit = { o: ByteRange; n: ByteRange } & (
  | { edit: "punct" }
  | { edit: "gap"; reminder: ByteRange[] }
  | { edit: "self-name"; form: "full" | "article" | "before-comma" | "planeswalker-subtype" | "before-epithet" | "first-word" }
  | { edit: "pronoun"; role: "subject" | "object" | "possessive" | "contraction" | "reflexive"; as: "neuter" | "name" }
  | { edit: "literal-name"; slot: "named" | "create" | "partner-with"; via: "catalog" | "own-name" }
);
