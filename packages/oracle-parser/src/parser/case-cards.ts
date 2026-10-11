// Case frame orientation is cosmetic; its printed keywords carry the rules.
// CR719.2/3 and702.169b-d distinguish an intervening-if solve trigger from
// the three different timings of a Solved gate.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, SolvedKeywordLine } from '../ast.ts';
import type { Token } from '../lexer.ts';
import { Cursor } from './cursor.ts';
import { parseCondition } from './refs.ts';

// A keyword may expand to an activated/triggered ability or even several
// different kinds (Suspend, Modular). Never infer its timing from its cost
// shape or assume that all keywords are static. This closed table supports
// only the single-class rules verified in CR702.2-21, 702.111, 702.122,
// 702.171, and the static evasion-ability definition in CR509.1b.
const SOLVED_KEYWORD_GATES: Readonly<Record<string, SolvedKeywordLine['keywords'][number]['solvedGate']>> = {
  deathtouch: 'continuous', defender: 'continuous', 'double strike': 'continuous',
  'first strike': 'continuous', flash: 'continuous', flying: 'continuous',
  haste: 'continuous', hexproof: 'continuous', 'hexproof from': 'continuous',
  indestructible: 'continuous', lifelink: 'continuous', menace: 'continuous',
  reach: 'continuous', shroud: 'continuous', trample: 'continuous', vigilance: 'continuous',
  protection: 'continuous', equip: 'activation', crew: 'activation', saddle: 'activation',
  ward: 'trigger-time',
};

export function startsCaseKeyword(tokens: readonly Token[]): boolean {
  return tokens[0]?.kind === 'word' && (tokens[0].value === 'solved'
    || tokens[0].value === 'to' && tokens[1]?.kind === 'word' && tokens[1].value === 'solve');
}

function hasTarget(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false;
  if ('ref' in value && (value.ref === 'target' || value.ref === 'any-target')) return true;
  if ('player' in value && (value.player === 'target-player' || value.player === 'target-opponent')) return true;
  return Object.values(value).some(hasTarget);
}

/** Callback consumes the complete child with the same token objects and
 * diagnostic session, so source IDs and original offsets are unchanged. */
export function parseCaseAbility(c: Cursor, parseChild: (tokens: Token[]) => Ability | null): Ability | null {
  return c.attempt(c => {
    if (c.words('to', 'solve')) {
      if (!c.punct('—')) return null;
      const condition = parseCondition(c);
      c.punct('.');
      if (!condition || !c.done() || hasTarget(condition)) return null;
      return { kind: 'case-solve', condition };
    }
    if (c.word('solved') === null || !c.punct('—')) return null;
    const ability = parseChild(c.tokens.slice(c.pos));
    if (!ability || ability.kind !== 'static' && ability.kind !== 'keywords' && ability.kind !== 'triggered'
      && ability.kind !== 'activated' && ability.kind !== 'loyalty') return null;
    if (ability.kind === 'keywords') {
      const keywords: SolvedKeywordLine['keywords'] = [];
      for (const keyword of ability.keywords) {
        const solvedGate = SOLVED_KEYWORD_GATES[keyword.keyword];
        if (!solvedGate) return null;
        keywords.push({ ...keyword, solvedGate });
      }
      c.pos = c.tokens.length;
      return { kind: 'case-solved', ability: { kind: 'keywords', keywords } };
    }
    c.pos = c.tokens.length;
    return { kind: 'case-solved', ability };
  });
}
