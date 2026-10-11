/** The facts needed to show a card. Structurally satisfied by the server's card rows. */
export interface CardInfo {
  oracle_id?: string;
  name: string;
  mana_cost: string | null;
  type_line: string;
  oracle_text: string;
  power: string | null;
  toughness: string | null;
  loyalty: string | null;
  /** The cheapest printing's price in cents; absent or null when unknown. */
  usd_cents?: number | null;
}
