/** The facts needed to show a card. Structurally satisfied by the server's card rows. */
export interface CardInfo {
  oracle_id?: string;
  name: string;
  image_small?: string | null;
  image_normal?: string | null;
  image_art_crop?: string | null;
  mana_cost: string | null;
  type_line: string;
  oracle_text: string;
  power: string | null;
  toughness: string | null;
  loyalty: string | null;
  /** The cheapest printing's price in cents; absent or null when unknown. */
  usd_cents?: number | null;
}
