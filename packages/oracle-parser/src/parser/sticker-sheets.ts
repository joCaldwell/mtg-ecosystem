// A sticker sheet is not a card and has no characteristics; its Oracle text
// lists stickers, each preceded by its ticket cost (CR 123.2, 123.3c,
// 107.17a). An ability sticker grants its ability to the object it is put on
// (123.7); a power/toughness sticker sets that object's power and toughness
// (123.8). Name and art stickers (123.6, 123.9) are not in the Oracle text.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, ParsedLine } from '../ast.ts';

/** The callback receives the original normalized starting line index. */
type BlockParser = (lines: readonly string[], startIndex: number) => ParsedLine[];

const STICKER_ROW = /^((?:\{TK\})+)\s+—\s+(.+)$/;
const POWER_TOUGHNESS = /^(\d+)\/(\d+)$/;

/** Abilities an object can have while a sticker is on it. Spell text and
 * card-structure blocks (chapters, level/station/Class bars, Attraction
 * visits) are not abilities a sticker grants. */
function isGrantableAbility(ability: Ability): boolean {
  if (['spell', 'additional-cost', 'chapter', 'class-level-bar', 'visit', 'sticker'].includes(ability.kind)) return false;
  if (ability.kind === 'max-speed' || ability.kind === 'infinity') return isGrantableAbility(ability.ability);
  return !(ability.kind === 'static' && (ability.effect.static === 'station-band' || ability.effect.static === 'level-band'));
}

/** Null means the face is not a sticker sheet. Every line of a sheet must be
 * a priced sticker; anything else fails rather than becoming an ability. */
export function parseStickerSheet(lines: readonly string[], parseBlocks: BlockParser, typeLine?: string): ParsedLine[] | null {
  if (typeLine?.trim() !== 'Stickers') return null;
  return lines.map((text, index): ParsedLine => {
    const row = STICKER_ROW.exec(text);
    if (!row) return { text, ok: false, error: 'sticker sheet line without a ticket cost' };
    const ticketCost = row[1].length / '{TK}'.length;
    const pt = POWER_TOUGHNESS.exec(row[2]);
    if (pt) return { text, ok: true, ability: { kind: 'sticker', ticketCost,
      sticker: { sticker: 'power-toughness', power: Number(pt[1]), toughness: Number(pt[2]) } } };
    const body = parseBlocks([row[2]], index);
    const parsed = body.length === 1 ? body[0] : undefined;
    if (!parsed?.ok) return { text, ok: false, error: `ability sticker: ${parsed && !parsed.ok ? parsed.error : 'not one ability'}` };
    if (!isGrantableAbility(parsed.ability)) return { text, ok: false, error: 'ability sticker requires an ability an object can have' };
    return { text, ok: true, ability: { kind: 'sticker', ticketCost, sticker: { sticker: 'ability', ability: parsed.ability } } };
  });
}
