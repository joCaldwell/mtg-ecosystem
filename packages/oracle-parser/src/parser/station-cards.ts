// A station symbol and every ability in its striation comprise one static
// ability gated on the source's charge counters, not several unconditional
// printed abilities (CR 721.2). The Oracle text prints each symbol as
// "N+ | <first ability>"; later lines up to the next symbol belong to the
// same striation, and nothing printed above the symbols does (721.4).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, ParsedLine, StaticEffect } from '../ast.ts';
import { finalizeCharacteristicDefinitions } from './characteristic-origin.ts';

type StationBand = Extract<StaticEffect, { static: 'station-band' }>;
/** The callback receives the original normalized starting line index. */
type BlockParser = (lines: readonly string[], startIndex: number) => ParsedLine[];

const STATION_ROW = /^(\d+)\+\s*\|\s*(.*)$/;

function isPermanentAbility(ability: Ability): boolean {
  if (ability.kind === 'spell' || ability.kind === 'additional-cost' || ability.kind === 'chapter') return false;
  if (ability.kind === 'max-speed' || ability.kind === 'infinity') return isPermanentAbility(ability.ability);
  if (ability.kind === 'static' && (ability.effect.static === 'station-band' || ability.effect.static === 'level-band')) return false;
  return ability.kind !== 'class-level-bar';
}

/** Oracle text carries no frame boxes. The striation holding the printed
 * power/toughness box (CR 721.2b) is the one the station reminder names:
 * "It's an artifact creature at N+." Without the reminder that fact is not
 * in the text at all, so it must be present for the card to be certified.
 * Returns the named threshold, null for a reminder that names none, and
 * undefined when the station line has no usable reminder. */
function creatureThreshold(rawText: string): number | null | undefined {
  const reminder = /^Station[ \t]*\(([^()\n]*)\)[ \t]*$/m.exec(rawText)?.[1];
  if (reminder === undefined || !/\bStation only as a sorcery\./.test(reminder)) return undefined;
  const named = [...reminder.matchAll(/\bIt['’]s an? (?:artifact )?creature at (\d+)\+\./g)];
  if (named.length > 1 || /\bcreature\b/.test(reminder.replace(/\bIt['’]s an? (?:artifact )?creature at \d+\+\./, '')
      .replace(/Tap another creature you control:/, ''))) return undefined;
  return named.length ? Number(named[0][1]) : null;
}

/** Null means the lines hold no station symbols, not a failed station card.
 * A recognized but malformed structure returns explicit failures, so that
 * ordinary line parsing cannot turn a striation's contents into
 * unconditional abilities or a dice results table. */
export function parseStationCard(lines: readonly string[], parseBlocks: BlockParser, rawText: string): ParsedLine[] | null {
  const keyword = lines.findIndex((line, index) => /^station$/i.test(line) && STATION_ROW.test(lines[index + 1] ?? ''));
  if (keyword < 0) return null;
  const first = keyword + 1;
  const results = parseBlocks(lines.slice(0, first), 0).map((line): ParsedLine => line.ok && !isPermanentAbility(line.ability)
    ? { text: line.text, ok: false, error: 'station-card top section requires permanent abilities' } : line);
  const fail = (error: string): ParsedLine[] => [...results, { text: lines.slice(first).join('\n'), ok: false, error }];
  const headers = lines.flatMap((line, index) => index >= first && STATION_ROW.test(line) ? [index] : []);
  const thresholds = headers.map(index => Number(STATION_ROW.exec(lines[index])![1]));
  // CR 721.1: one or two station symbols, each with a higher number.
  if (headers.length > 2 || thresholds.some((value, index) => !Number.isSafeInteger(value) || value < 1 ||
      (index > 0 && value <= thresholds[index - 1])))
    return fail('unsupported or unordered station symbols');
  const creatureAt = creatureThreshold(rawText);
  if (creatureAt === undefined || (creatureAt !== null && !thresholds.includes(creatureAt)))
    return fail('station reminder text does not identify the striation with the power/toughness box');
  for (let band = 0; band < headers.length; band++) {
    const start = headers[band], end = headers[band + 1] ?? lines.length;
    const text = lines.slice(start, end).join('\n');
    const body = parseBlocks([STATION_ROW.exec(lines[start])![2], ...lines.slice(start + 1, end)], start);
    const failed = body.find(line => !line.ok);
    if (failed && !failed.ok) {
      results.push({ text, ok: false, error: `station striation: ${failed.error}` });
      continue;
    }
    const abilities = body.flatMap(line => line.ok ? [line.ability] : []);
    if (!abilities.length || abilities.some(ability => !isPermanentAbility(ability))) {
      results.push({ text, ok: false, error: 'station striation requires complete permanent abilities' });
      continue;
    }
    const effect: StationBand = { static: 'station-band', threshold: thresholds[band], abilities,
      ...(creatureAt === thresholds[band] ? { creature: { basePT: 'printed-box' as const } } : {}) };
    results.push({ text, ok: true, ability: finalizeCharacteristicDefinitions({ kind: 'static', effect }) });
  }
  return results;
}
