import type { Actor } from "../actor.ts";
import type { Database } from "../sqlite.ts";
import { requireDeck, type Role } from "./service.ts";
import { tokenStatus, type TokenFacts } from "../token-catalog.ts";

export interface TokenProducer { oracle_id: string; name: string; quantity: number; role: Role }
export interface DeckToken extends TokenFacts { made_by: TokenProducer[]; producer_count: number }

/** One indexed join over the deck's non-maybe membership. No oracle parsing,
 * remote calls, full-catalog scan or per-card query on the serving path. */
export function deckTokens(db: Database, actor: Actor, deckId: number) {
  requireDeck(db, actor, deckId);
  const rows = db.prepare(`SELECT t.facts_json, c.oracle_id, c.name, dc.quantity, dc.role
    FROM deck_cards dc JOIN card_token_links l ON l.oracle_id = dc.oracle_id
    JOIN token_catalog t ON t.oracle_id = l.token_oracle_id
    JOIN cards c ON c.oracle_id = dc.oracle_id
    WHERE dc.deck_id = ? AND dc.maybe = 0 ORDER BY c.name, c.oracle_id`).all(deckId) as unknown as Array<TokenProducer & {facts_json: string}>;
  const tokens = new Map<string, DeckToken>();
  for (const { facts_json, ...producer } of rows) {
    const facts = JSON.parse(facts_json) as TokenFacts;
    let token = tokens.get(facts.oracle_id);
    if (!token) { token = { ...facts, made_by: [], producer_count: 0 }; tokens.set(facts.oracle_id, token); }
    token.made_by.push(producer);
    token.producer_count++;
  }
  const compare = (a: string, b: string) => a < b ? -1 : a > b ? 1 : 0;
  return { tokens: [...tokens.values()].sort((a, b) => compare(a.name, b.name) || compare(a.type_line, b.type_line) || compare(a.power ?? "", b.power ?? "") || compare(a.toughness ?? "", b.toughness ?? "") || compare(a.oracle_id, b.oracle_id)),
    tokens_status: tokenStatus(db) };
}
