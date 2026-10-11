// "While" is part of the matching event, not CR603.4's intervening-if.
// A condition on a departure event observes the appropriate CR603.10
// event snapshot; no gate is checked afresh during ability resolution.
// Saddled's designation and duration are intrinsic under CR702.171b.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition, Trigger, TriggerEventGate } from '../ast.ts';
import { Cursor } from './cursor.ts';
import { parseCondition, parseObjectRef } from './refs.ts';

function hasAttackSubject(event: Trigger): boolean {
  // This gate denotes one event participant. A grouped event first needs
  // an explicit eligibility predicate on its participants (before grouping),
  // rather than a single status test on the resulting group (CR603.2c).
  if (event.grouping) return false;
  return event.trigger === 'attacks' || event.trigger === 'any-of'
    && event.events.length > 0 && event.events.every(hasAttackSubject);
}

function hasTarget(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false;
  if ('ref' in value && (value.ref === 'target' || value.ref === 'any-target')) return true;
  if ('player' in value && (value.player === 'target-player' || value.player === 'target-opponent')) return true;
  return Object.values(value).some(hasTarget);
}

function parseExiledState(c: Cursor): Condition | null {
  return c.attempt(c => {
    if (c.words("it's", 'exiled')) return { condition: 'is-filter', what: { ref: 'it' }, filter: { zone: { zone: 'exile' } } };
    const what = parseObjectRef(c);
    if (!what || !c.words('is', 'exiled')) return null;
    return { condition: 'is-filter', what, filter: { zone: { zone: 'exile' } } };
  });
}

/** This hook consumes only a present while suffix on one complete introduced
 * trigger clause. Its caller retains following independently introduced
 * clauses. Unsupported conditions leave the entire attempt unchanged. */
export function parseTriggerGate(c: Cursor, event: Trigger): Trigger | null {
  return c.attempt(c => {
    if (c.word('while') === null) return null;
    // An ordinal qualified by a while clause needs a deliberate distinction
    // between all-event and qualifying-event history before support.
    if (event.occurrence) return null;
    let gate: TriggerEventGate;
    if (c.word('saddled') !== null) {
      if (!hasAttackSubject(event)) return null;
      gate = { gate: 'subject-status', status: 'saddled' };
    } else {
      const condition = parseCondition(c) ?? parseExiledState(c);
      // Matching the event precedes choosing this ability's targets (603.3d).
      // A fresh target declaration cannot be used as an event-time gate.
      if (!condition || hasTarget(condition)) return null;
      gate = { gate: 'condition', condition };
    }
    return { trigger: 'event-gated', event, gate };
  });
}
