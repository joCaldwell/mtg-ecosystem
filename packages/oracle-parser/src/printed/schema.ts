// Wire coordinates are half-open UTF-8 byte ranges with u32 endpoints.
export type ByteRange = [start: number, end: number];

export type Edit = { o: ByteRange; n: ByteRange } & (
  | { edit: "punct" }
  | { edit: "gap"; reminder: ByteRange[] }
  | { edit: "self-name"; form: "full" | "article" | "before-comma" | "planeswalker-subtype" | "before-epithet" | "first-word" }
  | { edit: "pronoun"; role: "subject" | "object" | "possessive" | "contraction" | "reflexive"; as: "neuter" | "name" }
  | { edit: "literal-name"; slot: "named" | "create" | "partner-with"; via: "catalog" | "own-name" }
);

/** Token indices address the line's tokens in physical order; end is exclusive. */
export type TokenRun = [line: number, start: number, end: number];
export type TokenAddress = [line: number, index: number];

export type PrintedToken = { n: ByteRange; o: ByteRange; grammarIndex: number } & (
  | { t: "word"; possessive?: "'s" | "'" }
  | { t: "selfref"; possessive?: true }
  | { t: "card-name" | "number" | "punct" | "symbol" }
);

export type PrintedLine = { sourceId: string; n: ByteRange } & (
  | { status: "lexed"; tokens: PrintedToken[] }
  | { status: "failed"; error: string; at: number }
);

export type ImplicitReason = "implied-singular-target" | "imperative-controller" | "default-zone" | "implied-singular-count";

export type PrintedNode = { path: string } & (
  | { basis: "recorded"; span: TokenRun[]; folds?: TokenRun[] }
  | { basis: "hull"; span: TokenRun[]; clauses?: TokenRun[] }
  | { basis: "alias"; of: string }
  | { basis: "implicit"; reason: ImplicitReason }
  | { basis: "unlocated" }
);

export interface RemovedReminder {
  fact: "removed-reminder";
  editIndex: number;
  o: ByteRange;
}

export type PrintedFact =
  | { fact: "reminder-citation"; role: "station-creature-threshold"; reminderIndex: number; o: ByteRange; field: string }
  | ({ at: string; tok: TokenRun; field: string | null } & (
    | { fact: "target"; targetIndex: number; scope: string; selectionId: string }
    | { fact: "introducer"; form: "target" | "a" | "another" | "each" | "all" | "any" | "up-to" | "the" | "numeral" | "x" }
    | { fact: "mention"; form: "source-name" | "gendered" | "this" | "that" | "those" | "it" | "they" | "you" | "its" | "their" | "your" | "this-way" | "that-many" | "the-chosen" | "the-rest" | "target" | "x"; possessive?: true }
    | { fact: "quantity"; form: "digits" | "word" | "article" | "x"; value: number | null }
    | { fact: "polarity"; form: "non-prefix" | "can't" | "don't" | "doesn't" | "isn't" | "not" | "no" | "only" | "without"; n?: ByteRange }
    | { fact: "optional"; form: "may" | "may-have" | "up-to" }
    | { fact: "exception"; form: "another" | "other" | "other-than" | "except" | "unless" | "instead" | "rather-than" }
    | { fact: "punct"; role: "sentence-end" | "comma" | "colon" | "dash" | "bullet" | "quote" | "inner-quote" | "other"; moved?: true }
  ));

export type PrintedBlock = { lines: ByteRange; n: ByteRange; facts: PrintedFact[] } & (
  | { status: "read"; nodes: PrintedNode[] }
  | { status: "failed"; error: string; at: number; location: "lex-error" | "token" | "end" }
);

export type PrintedFace = { sourceFaceIndex: number } & (
  | { status: "missing-oracle-text"; field: "absent" | "null" }
  | { status: "unmappable"; reason: "raw-tilde" | "raw-nul" | "invalid-unicode" | "composed-edit" | "unrestored-marker" }
  | { status: "read"; original: { bytes: number; sha256: string }; normalized: { bytes: number; sha256: string };
      edits: Edit[]; lines: PrintedLine[]; blocks: PrintedBlock[]; removedReminders: RemovedReminder[] }
);

/** generatedQuotes is projection metadata, absent from the wire. A literal
 * edit inserted its enclosure iff its original slice was unquoted. */
export interface PrintedCard {
  artifactKind: "oracle-printed-evidence";
  schemaVersion: 1;
  faces: PrintedFace[];
}
