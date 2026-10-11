// Modes are chosen as a spell/ability is put on the stack. Selected modes
// resolve in printed order; repeat permission, per-mode costs, and pawprint
// budgets change different parts of that procedure (CR 700.2a-i).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, Effect, ModalCount, ModalOption, Sentence } from '../ast.ts';
import type { Token } from '../lexer.ts';
import { wordNumber } from '../vocab.ts';
import { Cursor } from './cursor.ts';
import { parseCostList } from './costs.ts';

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
    const upTo = c.words('up', 'to');
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

export function parseModalEffect(c: Cursor, optionTokens: Token[][], parseBody: BodyParser,
  parseLabeled: (tokens: Token[]) => Ability | null): Modal | null {
  return c.attempt(c => {
    const keyword = c.word('spree', 'tiered') as Modal['keyword'] | null;
    let selection: ModalCount;
    let unchosen: Modal['unchosen'];
    let allowRepeat: true | undefined;
    if (keyword) {
      selection = keyword === 'spree' ? { atLeast: 1 } : { exactly: 1 };
    } else {
      if (c.word('choose') === null) return null;
      const parsed = count(c);
      if (!parsed) return null;
      selection = parsed;
      if (c.words('that', "hasn't", 'been', 'chosen')) {
        unchosen = { period: c.words('this', 'turn') ? 'turn' : 'source-object' };
      }
      if (c.punct('.')) {
        if (!c.done()) {
          if (!c.words('you', 'may', 'choose', 'the', 'same', 'mode', 'more', 'than', 'once') || !c.punct('.')) return null;
          allowRepeat = true;
        }
      } else if (!c.punct('—')) return null;
    }
    if (!c.done() || !optionTokens.length || (allowRepeat && unchosen)) return null;
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
        if (labeled?.kind !== 'spell' || !labeled.abilityWord || metadata.label)
          return c.fail(`modal option ${options.length + 1}: ${body.errorMessage()}`);
        options.push({ ...metadata, effects: labeled.effects, label: labeled.abilityWord });
      }
    }
    const minimum = 'exactly' in selection ? selection.exactly : 'min' in selection ? selection.min : 'atLeast' in selection ? selection.atLeast : 0;
    if (!allowRepeat && minimum > options.length) return c.fail('enough modal options');
    return { effect: 'modal', count: selection, options, ...(keyword ? { keyword } : {}),
      ...(allowRepeat ? { allowRepeat } : {}), ...(unchosen ? { unchosen } : {}) };
  });
}
