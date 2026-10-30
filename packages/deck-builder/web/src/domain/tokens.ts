// The tokens and emblems a deck can create, as the server computes them from
// Scryfall's related-card data. The client only arranges them: no predicted
// quantities, no guessed producers.

import type { DeckState, DeckToken, TokenProducer } from "../api/client.ts";
import type { DeckFilterMatches } from "./deckSearch.ts";
export type { DeckToken, TokenProducer };
export type TokensStatus = DeckState["tokens_status"];

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
