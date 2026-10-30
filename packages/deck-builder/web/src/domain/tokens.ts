// The tokens and emblems a deck can create, as the server computes them from
// Scryfall's related-card data. The client only arranges them: no predicted
// quantities, no guessed producers.
//
// These types mirror the contract of feat/deck-tokens exactly; they switch to
// imports from src/state.ts when feat/deck-tokens merges.

import type { DeckFilterMatches } from "./deckSearch.ts";

export interface TokenProducer {
  oracle_id: string;
  name: string;
  quantity: number;
  role: "card" | "commander" | "companion";
}

export interface DeckToken {
  oracle_id: string;
  name: string;
  kind: "token" | "emblem";
  type_line: string;
  oracle_text: string;
  power: string | null;
  toughness: string | null;
  /** Canonical WUBRG letters, "" for colourless. */
  colors: string;
  image_small: string | null;
  image_normal: string | null;
  image_art_crop: string | null;
  artist: string | null;
  made_by: TokenProducer[];
  producer_count: number;
}

export interface TokensStatus {
  /** False until the token catalog has been built: not the same as no tokens. */
  ready: boolean;
  as_of: string | null;
  source: "Scryfall";
}

export interface DeckTokens {
  tokens: DeckToken[];
  status: TokensStatus;
}

const NOT_READY: TokensStatus = { ready: false, as_of: null, source: "Scryfall" };

/** Read the tokens from a deck or public deck state, which may predate them. */
export function deckTokens(state: object): DeckTokens {
  const s = state as { tokens?: DeckToken[]; tokens_status?: TokensStatus };
  return { tokens: s.tokens ?? [], status: s.tokens_status ?? NOT_READY };
}

/** "1/1 · Creature — Soldier", "Artifact — Treasure", or "Emblem". Scryfall
 *  prefixes every token's type line with "Token"; the section already says so. */
export function tokenSummary(token: Pick<DeckToken, "kind" | "type_line" | "power" | "toughness">): string {
  if (token.kind === "emblem") return "Emblem";
  const type = token.type_line.replace(/(^|\/\/ )Token /g, "$1").trim() || token.type_line;
  const pt = token.power != null && token.toughness != null ? `${token.power}/${token.toughness}` : "";
  return [pt, type].filter(Boolean).join(" · ");
}

/** While a filter is active, a token shows when a card that makes it does. */
export function filterTokens(tokens: DeckToken[], filter: DeckFilterMatches | undefined): DeckToken[] {
  if (!filter?.active) return tokens;
  return tokens.filter((t) => t.made_by.some((p) => filter.cardIds.has(p.oracle_id)));
}
