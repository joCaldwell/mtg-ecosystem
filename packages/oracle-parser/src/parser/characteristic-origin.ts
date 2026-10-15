// CR 604.3a(2),(4), 613.4a-b: an ordinary ability grant cannot create a
// characteristic-defining ability. Abilities defined by token creation or
// a copy effect can qualify. Classify only after token-definition folding.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, StaticEffect, StaticModifier } from '../ast.ts';

type Origin = 'intrinsic' | 'granted';
function record(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

export function ordinaryCharacteristicDefinition(definition: StaticEffect): StaticEffect {
  if (definition.static === 'group') return { static: 'group', effects: definition.effects.map(ordinaryCharacteristicDefinition) };
  let modifier: StaticModifier;
  if (definition.static === 'cda-pt') {
    modifier = definition.stat === 'power'
      ? { modifier: 'base-pt', power: structuredClone(definition.value) }
      : definition.stat === 'toughness'
        ? { modifier: 'base-pt', toughness: structuredClone(definition.value) }
        : { modifier: 'base-pt', power: structuredClone(definition.value), toughness: structuredClone(definition.value) };
  } else if (definition.static === 'cda-colors') {
    modifier = { modifier: 'colors', operation: definition.operation, colors: structuredClone(definition.colors) };
  } else if (definition.static === 'cda-types') {
    modifier = { modifier: 'types', change: structuredClone(definition.change) };
  } else return definition;
  return { static: 'modify', what: structuredClone(definition.what), modifiers: [modifier] };
}

/** A compound self definition remains one ability; splitting its layer parts
 * into a group must never turn any constituent into an ordinary setter. */
export function intrinsicCharacteristicDefinition(definition: StaticEffect): StaticEffect {
  if (definition.static !== 'modify' || !['self', 'this'].includes(definition.what.ref)
    || definition.condition || definition.duration || definition.bindings) return definition;
  const parts: StaticEffect[] = [];
  const what = definition.what;
  for (const modifier of definition.modifiers) {
    if (modifier.condition || modifier.forEach || modifier.multiplier) return definition;
    if (modifier.modifier === 'colors') parts.push({ static: 'cda-colors', what,
      operation: modifier.operation, colors: modifier.colors });
    else if (modifier.modifier === 'types' && !modifier.change.cardTypes && !modifier.change.supertypes)
      parts.push({ static: 'cda-types', what, change: modifier.change });
    else if (modifier.modifier === 'base-pt') {
      if (modifier.power && modifier.toughness && JSON.stringify(modifier.power) === JSON.stringify(modifier.toughness))
        parts.push({ static: 'cda-pt', what, stat: 'both', value: modifier.power });
      else {
        if (modifier.power) parts.push({ static: 'cda-pt', what, stat: 'power', value: modifier.power });
        if (modifier.toughness) parts.push({ static: 'cda-pt', what, stat: 'toughness', value: modifier.toughness });
      }
    } else return definition;
  }
  return parts.length === 1 ? parts[0] : parts.length ? { static: 'group', effects: parts } : definition;
}

/** Return a new tree: parsing a quoted definition independently cannot know
 * whether its enclosing instruction grants it or defines a token with it. */
export function finalizeCharacteristicDefinitions(ability: Ability): Ability {
  function visit(value: unknown, origin: Origin): unknown {
    if (Array.isArray(value)) return value.map(item => visit(item, origin));
    if (!record(value)) return value;
    const result: Record<string, unknown> = {};
    for (const [key, child] of Object.entries(value)) {
      // An ability's descendants are ordinary grants unless token creation
      // or a copy exception establishes new copiable characteristics.
      // Quoted intrinsic definitions retain that origin only at their root;
      // any abilities they themselves grant are ordinary grants again.
      const childOrigin = typeof value.kind === 'string' ? 'granted' : origin;
      result[key] = visit(child, value.effect === 'create-token' && key === 'token'
        || value.static === 'entry-copy' && key === 'changes' || key === 'copyExceptions' ? 'intrinsic' : childOrigin);
    }
    if (value.kind === 'static' && origin === 'granted' && record(value.effect)) {
      result.effect = ordinaryCharacteristicDefinition(result.effect as StaticEffect);
    }
    return result;
  }
  return visit(ability, 'intrinsic') as Ability;
}
