// Counter trigger patterns, not counter-placement instructions or replacements.
// CR122.2 distinguishes removal from counters ceasing to exist; 122.6 includes
// entering with counters and specifies the placing actor; 122.7 defines Nth
// counter threshold crossings. CR603.2c/700.1 distinguish event groupings.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// https://magic.wizards.com/en/news/feature/phyrexia-all-will-be-one-release-notes
import type { CounterChangeMatch, CounterChangeRecipient, CounterChangeSingleRecipient, CounterSpec, ObjectRef, Trigger } from '../ast.ts';
import { Cursor } from './cursor.ts';
import { isPluralPlayerRef, parseCounterSpec, parseFilter, parseObjectRef, parsePlayerRef } from './refs.ts';

type CounterPattern = { counter: CounterSpec | 'any'; match: CounterChangeMatch; plural: boolean };
const ORDINALS = ['first', 'second', 'third', 'fourth', 'fifth', 'sixth', 'seventh', 'eighth', 'ninth', 'tenth',
  'eleventh', 'twelfth', 'thirteenth', 'fourteenth', 'fifteenth', 'sixteenth', 'seventeenth', 'eighteenth', 'nineteenth', 'twentieth'];

function hasTarget(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false;
  if ('ref' in value && (value.ref === 'target' || value.ref === 'any-target')) return true;
  if ('player' in value && (value.player === 'target-player' || value.player === 'target-opponent')) return true;
  return Object.values(value).some(hasTarget);
}

function counterPattern(c: Cursor): CounterPattern | null {
  return c.attempt(c => {
    let match: CounterChangeMatch;
    let plural = false;
    if (c.words('one', 'or', 'more')) {
      match = { counters: 'one-or-more', recipients: 'each' };
      plural = true;
    } else if (c.word('a', 'an') !== null) match = { counters: 'each', recipients: 'each' };
    else if (c.word('the') !== null) {
      if (c.word('last') !== null) match = { transition: 'last-removed' };
      else {
        const ordinal = c.word(...ORDINALS);
        if (!ordinal) return null;
        match = { transition: 'crosses-upward', threshold: ORDINALS.indexOf(ordinal) + 1 };
      }
    } else return null;
    const counter = c.isWord('counter', 'counters') ? 'any' : parseCounterSpec(c);
    if (!counter || c.word(plural ? 'counters' : 'counter') === null) return null;
    return { counter, match, plural };
  });
}

function singleObject(ref: ObjectRef): boolean {
  switch (ref.ref) {
    case 'self': case 'this': case 'it': case 'enchanted': case 'equipped': return true;
    case 'that': return !ref.plural;
    case 'filter': return ref.count?.amount === 'fixed' && ref.count.value === 1;
    default: return false;
  }
}

function singleRecipient(c: Cursor, omittedArticle = false): CounterChangeSingleRecipient | null {
  return c.attempt(c => {
    const who = c.attempt(parsePlayerRef) ?? (omittedArticle && c.word('player') !== null ? { player: 'a-player' as const } : null);
    if (who) return !isPluralPlayerRef(who) && !hasTarget(who) ? { recipient: 'player', who } : null;
    const what = parseObjectRef(c);
    // Bare plurals such as Bioessence Hydra's planeswalkers require an
    // independently established group interpretation. Do not turn a whole
    // set into a single recipient or infer the grouping from the body.
    return what && singleObject(what) && !hasTarget(what) ? { recipient: 'object', what } : null;
  });
}

function recipients(c: Cursor): { on: CounterChangeRecipient; grouped: boolean } | null {
  return c.attempt(c => {
    if (c.words('one', 'or', 'more')) {
      const filter = parseFilter(c);
      if (!filter || hasTarget(filter)) return null;
      return { on: { recipient: 'object', what: { ref: 'each', filter } }, grouped: true };
    }
    const first = singleRecipient(c);
    if (!first) return null;
    if (c.word('or') === null) return { on: first, grouped: false };
    const second = singleRecipient(c, true);
    if (!second) return null;
    const alternatives: [CounterChangeSingleRecipient, CounterChangeSingleRecipient, ...CounterChangeSingleRecipient[]] = [first, second];
    while (c.word('or') !== null) {
      const next = singleRecipient(c, true);
      if (!next) return null;
      alternatives.push(next);
    }
    return { on: { recipient: 'any-of', recipients: alternatives }, grouped: false };
  });
}

/** Called after the `When/Whenever` opener. No targets are chosen while
 * matching an event: the enclosing triggered ability chooses its body
 * targets later, when put on the stack (CR603.3d). */
export function parseCounterChangeTrigger(c: Cursor): Trigger | null {
  return c.attempt(c => {
    const active = c.attempt(c => {
      const by = parsePlayerRef(c);
      if (!by || hasTarget(by) || isPluralPlayerRef(by)) return null;
      const verb = c.word('put', 'puts', 'remove', 'removes');
      return verb && { by, direction: verb === 'put' || verb === 'puts' ? 'put' as const : 'remove' as const };
    });
    const pattern = counterPattern(c);
    if (!pattern) return null;
    let direction: 'put' | 'remove';
    if (active) direction = active.direction;
    else {
      if (c.word(pattern.plural ? 'are' : 'is') === null) return null;
      const verb = c.word('put', 'removed');
      if (!verb) return null;
      direction = verb === 'put' ? 'put' : 'remove';
    }
    if (c.word(direction === 'put' ? 'on' : 'from') === null) return null;
    const recipient = recipients(c);
    if (!recipient) return null;
    let match = pattern.match;
    if ('transition' in match) {
      if (recipient.grouped || recipient.on.recipient === 'any-of'
        || (match.transition === 'crosses-upward') !== (direction === 'put')) return null;
    } else if (recipient.grouped) {
      // The singular-counter/collective-recipient combination needs its own
      // exact occurrence definition; it isn't either supported pattern.
      if (match.counters !== 'one-or-more') return null;
      match = { counters: 'one-or-more', recipients: 'one-or-more' };
    }
    return { trigger: 'counter-change', direction, counter: pattern.counter, on: recipient.on,
      ...(active ? { by: active.by } : {}), match };
  });
}
