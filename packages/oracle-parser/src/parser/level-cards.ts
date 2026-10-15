// A LEVEL symbol, its P/T box, and every ability in that striation comprise
// one static ability, not several unconditional printed abilities (CR 711).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// Noncreature levelers omit P/T, as confirmed by the official MB2 notes:
// https://magic.wizards.com/en/news/feature/mystery-booster-2-release-notes
import type { Ability, ParsedLine, StaticEffect } from '../ast.ts';
import { LexError, type Token } from '../lexer.ts';
import { finalizeCharacteristicDefinitions } from './characteristic-origin.ts';
import { Cursor } from './cursor.ts';
import { parseCostList } from './costs.ts';

type LevelBand = Extract<StaticEffect, { static: 'level-band' }>;
/** The callback receives the original normalized starting line index. It
 * must preserve that index when lexing nested modal options and table rows. */
type BlockParser = (lines: readonly string[], startIndex: number) => ParsedLine[];

function startsLevelSymbol(text: string): boolean {
  return /^level\b(?!\s+up\b)/i.test(text);
}

function range(text: string): LevelBand['range'] | null {
  const match = /^level\s+(\d+)\s*(?:-\s*(\d+)|(\+))$/i.exec(text);
  if (!match) return null;
  const min = Number(match[1]), max = match[2] === undefined ? undefined : Number(match[2]);
  if (!Number.isSafeInteger(min) || max !== undefined && (!Number.isSafeInteger(max) || max < min)) return null;
  return { min, ...(max === undefined ? {} : { max }) };
}

function powerToughness(text: string | undefined): LevelBand['basePT'] | null {
  const match = text && /^([+-]?\d+)\s*\/\s*([+-]?\d+)$/.exec(text);
  if (!match) return null;
  const power = Number(match[1]), toughness = Number(match[2]);
  return Number.isSafeInteger(power) && Number.isSafeInteger(toughness) ? { power, toughness } : null;
}

function isPermanentAbility(ability: Ability): boolean {
  if (ability.kind === 'spell' || ability.kind === 'additional-cost' || ability.kind === 'chapter') return false;
  if (ability.kind === 'max-speed' || ability.kind === 'infinity') return isPermanentAbility(ability.ability);
  return true;
}

/** Null means no level symbol was found, not a failed leveler. Recognized
 * malformed structures return explicit failures so ordinary line parsing
 * cannot turn their contents into unconditional abilities. Layout and card
 * names never establish these boundaries. */
export function parseLevelCard(lines: readonly string[], parseBlocks: BlockParser, typeLine?: string): ParsedLine[] | null {
  const headers = lines.flatMap((text, index) => startsLevelSymbol(text) ? [index] : []);
  if (!headers.length) return null;
  const first = headers[0];
  const results = permanentPrefix(parseBlocks(lines.slice(0, first), 0));
  const fail = (error: string): ParsedLine[] => [...results, { text: lines.slice(first).join('\n'), ok: false, error }];
  // CR 711.1: two symbols, a bounded striation followed by an open range.
  if (headers.length !== 2) return fail('leveler requires two complete level-symbol striations');
  const ranges = headers.map(index => range(lines[index]));
  const lower = ranges[0], upper = ranges[1];
  if (!lower || !upper || lower.max === undefined || upper.max !== undefined || upper.min <= lower.max)
    return fail('unsupported, unordered, or overlapping level-symbol ranges');
  const cardTypes = typeLine?.split(/—|\s-\s/u, 1)[0] ?? '';
  const noncreaturePermanent = /\b(?:Artifact|Battle|Enchantment|Land|Planeswalker)\b/.test(cardTypes)
    && !/\b(?:Creature|Instant|Sorcery)\b/.test(cardTypes);
  for (let bandIndex = 0; bandIndex < headers.length; bandIndex++) {
    const start = headers[bandIndex], end = headers[bandIndex + 1] ?? lines.length;
    const text = lines.slice(start, end).join('\n');
    const basePT = powerToughness(lines[start + 1]);
    if (!basePT && !noncreaturePermanent) {
      results.push({ text, ok: false, error: 'level symbol requires numeric power/toughness, or proven noncreature source metadata' });
      continue;
    }
    const bodyStart = start + (basePT ? 2 : 1);
    const body = parseBlocks(lines.slice(bodyStart, end), bodyStart);
    const failed = body.find(line => !line.ok);
    if (failed && !failed.ok) {
      results.push({ text, ok: false, error: `level striation: ${failed.error}` });
      continue;
    }
    const abilities: Ability[] = [];
    for (const line of body) if (line.ok) abilities.push(line.ability);
    if ((!basePT && !abilities.length) || abilities.some(ability => !isPermanentAbility(ability))) {
      results.push({ text, ok: false, error: 'level striation requires permanent abilities or printed power/toughness' });
      continue;
    }
    const effect: LevelBand = { static: 'level-band', range: bandIndex === 0 ? lower : upper,
      ...(basePT ? { basePT } : {}), abilities };
    results.push({ text, ok: true, ability: finalizeCharacteristicDefinitions({ kind: 'static', effect }) });
  }
  return results;
}

function permanentPrefix(results: ParsedLine[]): ParsedLine[] {
  return results.map(line => line.ok && line.ability.kind === 'spell'
    ? { text: line.text, ok: false, error: 'level-card top section requires permanent abilities, not resolving spell instructions' }
    : line);
}

/** The bar's costs and children are printed abilities at every Class level;
 * the current designation controls activation and application (716.2/3).
 * Tokenization uses the original global line index, including cost targets.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export function parseClassCard(lines: readonly string[], parseBlocks: BlockParser,
  tokenize: (text: string, lineIndex: number) => Token[]): ParsedLine[] | null {
  const headers: { index: number; tokens: Token[]; colon: number }[] = [];
  for (let index = 0; index < lines.length; index++) {
    if (!/:\s*level\b/i.test(lines[index])) continue;
    let tokens: Token[];
    try { tokens = tokenize(lines[index], index); }
    catch (error) {
      if (!(error instanceof LexError)) throw error;
      return [{ text: lines.join('\n'), ok: false, error: `class level bar: ${error.message} at position ${error.position}` }];
    }
    let quoted = false;
    for (let i = 0; i < tokens.length; i++) {
      const token = tokens[i];
      if (token.kind === 'punct' && token.value === '"') quoted = !quoted;
      if (quoted || token.kind !== 'punct' || token.value !== ':') continue;
      const next = tokens[i + 1];
      if (next?.kind === 'word' && next.value === 'level') headers.push({ index, tokens, colon: i });
      break;
    }
  }
  if (!headers.length) return null;
  const first = headers[0].index;
  const results = permanentPrefix(parseBlocks(lines.slice(0, first), 0));
  const fail = (error: string): ParsedLine[] => [...results, { text: lines.slice(first).join('\n'), ok: false, error }];
  if (headers.length !== 2) return fail('Class requires two complete level-bar sections');
  const bars: Extract<Ability, { kind: 'class-level-bar' }>[] = [];
  for (const header of headers) {
    const costCursor = new Cursor(header.tokens.slice(0, header.colon));
    const costs = parseCostList(costCursor);
    const c = new Cursor(header.tokens.slice(header.colon + 1));
    if (!costs || !costCursor.done() || !c.word('level')) return fail('unsupported Class level-bar activation cost');
    const level = c.number();
    if (level === null || !Number.isSafeInteger(level) || level < 2 || !c.done()) return fail('unsupported Class level designation');
    bars.push({ kind: 'class-level-bar', level, costs, abilities: [] });
  }
  if (bars[0].level >= bars[1].level) return fail('Class level bars must be in increasing order');
  for (let i = 0; i < headers.length; i++) {
    const start = headers[i].index, end = headers[i + 1]?.index ?? lines.length;
    const text = lines.slice(start, end).join('\n');
    const children = parseBlocks(lines.slice(start + 1, end), start + 1);
    const failed = children.find(child => !child.ok);
    if (failed && !failed.ok) {
      results.push({ text, ok: false, error: `Class level section: ${failed.error}` });
      continue;
    }
    const abilities = children.flatMap(child => child.ok ? [child.ability] : []);
    if (!abilities.length || abilities.some(ability => !isPermanentAbility(ability) || ability.kind === 'class-level-bar')) {
      results.push({ text, ok: false, error: 'Class level section requires complete permanent abilities' });
      continue;
    }
    results.push({ text, ok: true, ability: finalizeCharacteristicDefinitions({ ...bars[i], abilities }) });
  }
  return results;
}
