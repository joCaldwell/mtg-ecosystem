// Modes are chosen as a spell/ability is put on the stack. Selected modes
// resolve in printed order; repeat permission, per-mode costs, and pawprint
// budgets change different parts of that procedure (CR 700.2a-i).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, Effect, ModalCount, ModalOption, PlayerRef, Sentence } from '../ast.ts';
import type { Token } from '../lexer.ts';
import { wordNumber } from '../vocab.ts';
import { Cursor } from './cursor.ts';
import { parseCostList } from './costs.ts';
import { parseActivationRules } from './restrictions.ts';
import { parseCondition, parsePlayerRef } from './refs.ts';
import { isKeywordName } from './keywords.ts';

type Modal = Extract<Effect, { effect: 'modal' }>;
type BodyParser = (c: Cursor) => Sentence[] | null;

export function hasCastingCostModes(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false;
  if (Array.isArray(value)) return value.some(hasCastingCostModes);
  if ('effect' in value && value.effect === 'modal' && 'keyword' in value &&
      (value.keyword === 'spree' || value.keyword === 'tiered')) return true;
  return Object.values(value).some(hasCastingCostModes);
}

function pawprint(c: Cursor): boolean {
  const token = c.peek();
  if (token?.kind !== 'symbol' || token.value !== 'P') return false;
  c.pos++;
  return true;
}

function number(c: Cursor): number | null {
  const t = c.peek();
  const n = t?.kind === 'number' ? t.value : t?.kind === 'word' ? wordNumber(t.value) : undefined;
  if (n === undefined || !Number.isInteger(n) || n < 0) return null;
  c.pos++;
  return n;
}

function count(c: Cursor): ModalCount | null {
  return c.attempt(c => {
    if (c.word('x') !== null) return { variable: 'x' };
    if (c.words('any', 'number')) return { atLeast: 0 };
    const upTo = c.words('up', 'to');
    if (upTo && c.word('x') !== null) return { variable: 'x', upTo: true };
    if (upTo && c.words('that', 'many')) return { variable: 'that-many', upTo: true };
    const n = number(c);
    if (n === null) return c.fail('modal count');
    if (pawprint(c)) {
      if (!upTo || !c.words('worth', 'of', 'modes')) return null;
      return { weightBudget: n };
    }
    if (upTo) return { min: 0, max: n };
    if (c.words('or', 'more')) return { atLeast: n };
    if (c.words('or', 'both')) return { min: n, max: 2 };
    return { exactly: n };
  });
}

/** "If [condition] [as you cast this spell], [you may] choose both/N
 * instead". Modes of a spell are chosen as it is cast (CR 601.2b), so the
 * printed "as you cast this spell" names the moment this count is decided. */
function conditionalCount(c: Cursor, base: ModalCount): (NonNullable<Modal['conditionalCount']> & { both: boolean }) | null {
  return c.attempt(c => {
    if (!('exactly' in base) || c.word('if') === null) return null;
    const condition = parseCondition(c);
    if (!condition) return null;
    c.words('as', 'you', 'cast', 'this', 'spell');
    if (!c.punct(',')) return null;
    const optional = c.words('you', 'may');
    if (c.word('choose') === null) return null;
    // "choose any number instead": every mode may be chosen (CR 700.2b).
    if (!optional && c.words('any', 'number', 'instead')) return { condition, count: { atLeast: 0 }, both: false };
    const both = c.word('both') !== null;
    const n = both ? 2 : number(c);
    if (n === null || n <= base.exactly || c.word('instead') === null) return null;
    return { condition, count: optional ? { min: base.exactly, max: n } : { exactly: n }, both };
  });
}

function hasTargetPlayer(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false;
  if (Array.isArray(value)) return value.some(hasTargetPlayer);
  if ('player' in value && (value.player === 'target-player' || value.player === 'target-opponent')) return true;
  return Object.values(value).some(hasTargetPlayer);
}

/** A paid mode may have a flavor label before its cost. The label cannot
 * substitute for the cost or for a completely parsed resolution body. */
function paidMode(c: Cursor): { additionalCost: NonNullable<ModalOption['additionalCost']>; label?: string } | null {
  return c.attempt(c => {
    const direct = c.attempt(c => {
      const additionalCost = parseCostList(c);
      return additionalCost && c.punct('—') ? { additionalCost } : null;
    });
    if (direct) return direct;
    const label: string[] = [];
    while (c.peek()?.kind === 'word') {
      const word = c.anyWord();
      if (!word) return null;
      label.push(word.raw);
    }
    if (!label.length || !c.punct('—')) return null;
    const additionalCost = parseCostList(c);
    return additionalCost && c.punct('—') ? { label: label.join(' '), additionalCost } : null;
  });
}

/** "• Temba, His Arms Wide — …", "• Take 59 Flights of Stairs — …",
 * "• Combine Powers! — …": a flavor word before a mode (CR 207.2d) has no
 * rules meaning and may contain numbers and punctuation that an ability-word
 * line prefix cannot. It ends at the option's first dash, never at a sentence
 * boundary, and keywords keep their own meaning. */
function flavorLabel(tokens: Token[], isKeywordName: (name: string) => boolean): { label: string; rest: Token[] } | null {
  const dash = tokens.findIndex(token => token.kind === 'punct' && token.value === '—');
  if (dash < 1 || dash > 8 || tokens[0].kind !== 'word') return null;
  let label = '';
  for (const token of tokens.slice(0, dash)) {
    if (token.kind === 'word') label += `${label ? ' ' : ''}${token.raw}${token.possessive ? token.possessiveSuffix ?? "'s" : ''}`;
    else if (token.kind === 'number') label += `${label ? ' ' : ''}${token.raw}`;
    else if (token.kind === 'punct' && (token.value === '!' || token.value === ',')) label += token.value;
    else return null;
  }
  const name = label.toLowerCase();
  if (isKeywordName(name) || name === 'visit') return null;
  return { label, rest: tokens.slice(dash + 1) };
}

export function parseModalEffect(c: Cursor, optionTokens: Token[][], parseBody: BodyParser,
  parseLabeled: (tokens: Token[]) => Ability | null): Modal | null {
  return c.attempt(c => {
    const keyword = c.word('spree', 'tiered') as Modal['keyword'] | null;
    let selection: ModalCount;
    let unchosen: Modal['unchosen'];
    let allowRepeat: true | undefined;
    let random = false;
    let activationTail = false;
    let distinctTargetPlayers = false;
    let conditional: ReturnType<typeof conditionalCount> | undefined;
    let chooser: PlayerRef | undefined;
    if (keyword) {
      selection = keyword === 'spree' ? { atLeast: 1 } : { exactly: 1 };
    } else {
      // CR 700.2e: another player chooses the modes.
      chooser = c.attempt(c => {
        const who = parsePlayerRef(c);
        return who && (who.player === 'an-opponent' || who.player === 'target-opponent') && c.word('chooses') !== null ? who : null;
      }) ?? undefined;
      if (!chooser && c.word('choose') === null) return null;
      const parsed = count(c);
      if (!parsed) return null;
      selection = parsed;
      random = c.words('at', 'random');
      if (c.words('that', "hasn't", 'been', 'chosen')) {
        unchosen = { period: c.words('this', 'turn') ? 'turn' : 'source-object' };
      }
      // CR 602.5: "Choose one. Activate only ..." ends the modal instruction;
      // the complete restriction tail stays for the enclosing ability block.
      activationTail = c.attempt(c => {
        if (!c.punct('.') || !c.isWord('activate')) return null;
        const tail = new Cursor(c.tokens.slice(c.pos));
        return parseActivationRules(tail) && tail.done() ? true : null;
      }) === true;
      if (activationTail) c.pos--;
      else if (c.punct('.')) {
        if (!c.done()) {
          if (c.words('you', 'may', 'choose', 'the', 'same', 'mode', 'more', 'than', 'once')) allowRepeat = true;
          else if (c.words('each', 'mode', 'must', 'target', 'a', 'different', 'player')) distinctTargetPlayers = true;
          else {
            const upgrade = conditionalCount(c, selection);
            if (!upgrade) return null;
            conditional = upgrade;
          }
          // "…, choose any number instead—" introduces the bullets itself.
          if (!c.punct('.') && !(conditional && c.punct('—'))) return null;
        }
      } else if (!c.punct('—')) return null;
    }
    if ((!c.done() && !activationTail) || !optionTokens.length || (allowRepeat && unchosen)) return null;
    const options: ModalOption[] = [];
    for (const tokens of optionTokens) {
      const body = new Cursor(tokens);
      let metadata: Omit<ModalOption, 'effects'> = {};
      if (keyword) {
        const paid = paidMode(body);
        if (!paid) return c.fail(`modal option ${options.length + 1}: complete additional cost`);
        metadata = paid;
      } else if ('weightBudget' in selection) {
        let weight = 0;
        while (pawprint(body)) weight++;
        if (!weight || !body.punct('—')) return c.fail(`modal option ${options.length + 1}: pawprint weight`);
        metadata = { weight };
      }
      const start = body.pos;
      const effects = parseBody(body);
      if (effects && body.done()) options.push({ ...metadata, effects });
      else {
        const labeled = parseLabeled(tokens.slice(start));
        if (labeled?.kind === 'spell' && labeled.abilityWord && !metadata.label) {
          options.push({ ...metadata, effects: labeled.effects, label: labeled.abilityWord });
          continue;
        }
        const flavor = metadata.label ? null : flavorLabel(tokens.slice(start), isKeywordName);
        const flavored = flavor && new Cursor(flavor.rest);
        const flavorEffects = flavored && parseBody(flavored);
        if (!flavor || !flavorEffects || !flavored.done())
          return c.fail(`modal option ${options.length + 1}: ${body.errorMessage()}`);
        options.push({ ...metadata, effects: flavorEffects, label: flavor.label });
      }
    }
    const minimum = 'exactly' in selection ? selection.exactly : 'min' in selection ? selection.min : 'atLeast' in selection ? selection.atLeast : 0;
    if (!allowRepeat && minimum > options.length) return c.fail('enough modal options');
    const upgraded = !conditional ? 0 : 'max' in conditional.count ? conditional.count.max : 'exactly' in conditional.count ? conditional.count.exactly : 0;
    if (upgraded > options.length) return c.fail('enough modal options');
    if (conditional?.both && options.length !== 2) return c.fail("exactly two modes for 'both'");
    if (distinctTargetPlayers && !options.every(option => hasTargetPlayer(option.effects)))
      return c.fail('a targeted player in every mode');
    return { effect: 'modal', count: selection, options, ...(keyword ? { keyword } : {}),
      ...(allowRepeat ? { allowRepeat } : {}), ...(unchosen ? { unchosen } : {}), ...(random ? { random: true as const } : {}),
      ...(conditional ? { conditionalCount: { condition: conditional.condition, count: conditional.count } } : {}),
      ...(distinctTargetPlayers ? { distinctTargetPlayers: true as const } : {}), ...(chooser ? { chooser } : {}) };
  });
}
