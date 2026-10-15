// Putting stickers on objects (CR 123.3). The sticker itself is chosen by
// the acting player from the sheets they have access to as the instruction
// is performed; the text only says which kind and which object.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef, PlayerRef, StickerKind } from '../ast.ts';
import type { Cursor } from './cursor.ts';

export interface StickerActionDeps {
  actor: (c: Cursor, imperative: string, conjugated: string) => PlayerRef | null;
  objectRef: (c: Cursor) => ObjectRef | null;
}

/** "a sticker", "a name sticker", "an ability sticker", "an art sticker",
 * "a power and toughness sticker" (CR 123.1). One sticker only. */
export function parseStickerKind(c: Cursor): StickerKind | null {
  return c.attempt((c): StickerKind | null => {
    const article = c.word('a', 'an');
    if (!article) return null;
    let kind: StickerKind = 'any';
    if (c.words('power', 'and', 'toughness')) kind = 'power-toughness';
    else {
      const word = c.word('name', 'ability', 'art');
      if (word) kind = word as StickerKind;
    }
    if ((article === 'an') !== (kind === 'ability' || kind === 'art')) return null;
    return c.word('sticker') ? kind : null;
  });
}

/** "[player] put(s) a [kind] sticker on <object>" */
export function parseStickerAction(c: Cursor, deps: StickerActionDeps): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const who = deps.actor(c, 'put', 'puts');
    if (!who) return null;
    const sticker = parseStickerKind(c);
    if (!sticker || !c.word('on')) return null;
    const on = deps.objectRef(c);
    return on ? [{ effect: 'put-sticker', who, sticker, on }] : null;
  });
}
