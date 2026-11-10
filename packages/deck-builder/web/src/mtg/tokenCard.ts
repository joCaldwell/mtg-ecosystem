import type { DeckToken } from "../domain/tokens.ts";
import type { CardInfo } from "./CardInfo.ts";

/** A token as the hover card and CardSheet draw a card: its own stored image
 *  and text, no cost and no price. */
export function tokenCardInfo(token: DeckToken): CardInfo {
  return {
    oracle_id: token.oracle_id,
    name: token.name,
    image_small: token.image_small,
    image_normal: token.image_normal,
    image_art_crop: token.image_art_crop,
    mana_cost: null,
    type_line: token.type_line,
    oracle_text: token.oracle_text,
    power: token.power,
    toughness: token.toughness,
    loyalty: null,
  };
}
