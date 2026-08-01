// Static (continuous) abilities: anthems, CDAs, enters-tapped, can't-effects.

import type { Amount, Condition, ObjectRef, StaticEffect, StaticModifier } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseCantAction, parseGainedAbilities, parsePtModifier } from "./effects.ts";
import {
  parseAmount, parseCondition, parseCount, parseCounterSpec, parseDuration,
  parseEqualTo, parseFilter, parseObjectRef,
} from "./refs.ts";

function parseStaticSubject(c: Cursor): ObjectRef | null {
  return parseObjectRef(c);
}

/** "get +1/+1 and have flying" — one or more modifiers joined by "and". */
function parseStaticModifiers(c: Cursor): StaticModifier[] | null {
  const modifiers: StaticModifier[] = [];
  for (;;) {
    const verb = c.word("get", "gets", "have", "has", "gain", "gains");
    if (!verb) return modifiers.length ? modifiers : null;
    if (verb.startsWith("get")) {
      const pt = parsePtModifier(c);
      if (!pt) return null;
      modifiers.push({ modifier: "pt", power: pt.power, toughness: pt.toughness });
    } else {
      const abilities = parseGainedAbilities(c);
      if (!abilities) return null;
      modifiers.push({ modifier: "abilities", abilities });
    }
    if (c.word("and") === null) break;
  }
  return modifiers;
}

export function parseStatic(c: Cursor): StaticEffect | null {
  // "As long as <condition>, <static>"
  const conditioned = c.attempt((c): StaticEffect | null => {
    if (!c.words("as", "long", "as")) return null;
    const condition = parseCondition(c);
    if (!condition || !c.punct(",")) return null;
    const inner = parseStatic(c);
    if (!inner) return null;
    if (inner.static === "modify") return { ...inner, condition };
    if (inner.static === "cant") return { ...inner, condition };
    return null;
  });
  if (conditioned) return conditioned;

  // "~ enters (the battlefield)? tapped."
  const entersTapped = c.attempt((c): StaticEffect | null => {
    const what = parseStaticSubject(c);
    if (!what) return null;
    if (c.word("enters") === null) return null;
    c.words("the", "battlefield");
    if (c.word("tapped") === null) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "enters-tapped", what };
  });
  if (entersTapped) return entersTapped;

  // "~ enters with N <kind> counters on it."
  const entersWithCounters = c.attempt((c): StaticEffect | null => {
    const what = parseStaticSubject(c);
    if (!what) return null;
    if (c.word("enters") === null) return null;
    c.words("the", "battlefield");
    if (c.word("with") === null) return null;
    const count = parseCount(c);
    if (!count) return null;
    const counter = parseCounterSpec(c);
    if (!counter) return null;
    if (c.word("counter", "counters") === null) return null;
    c.words("on", "it");
    c.punct(".");
    if (!c.done()) return null;
    return { static: "enters-with-counters", what, counter, count };
  });
  if (entersWithCounters) return entersWithCounters;

  // CDA: "~'s power and toughness are each equal to <amount>." /
  //      "~'s power is equal to <amount>."
  const cda = c.attempt((c): StaticEffect | null => {
    let what: ObjectRef | null = null;
    if (c.selfrefPossessive()) what = { ref: "self" };
    else if (c.word("its") !== null) what = { ref: "it" };
    if (!what) return null;
    let stat: "power" | "toughness" | "both";
    if (c.words("power", "and", "toughness")) stat = "both";
    else {
      const w = c.word("power", "toughness");
      if (!w) return null;
      stat = w as "power" | "toughness";
    }
    if (c.word("is", "are") === null) return null;
    c.word("each");
    const value = parseEqualTo(c) ?? parseAmount(c);
    if (!value) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "cda-pt", what, stat, value };
  });
  if (cda) return cda;

  // "<subject> can't <action> (as long as <condition>)?"
  const cant = c.attempt((c): StaticEffect | null => {
    const what = parseStaticSubject(c);
    if (!what) return null;
    if (c.word("can't") === null) return null;
    const action = parseCantAction(c);
    if (!action) return null;
    let condition: Condition | undefined;
    const cond = c.attempt((c): Condition | null => {
      if (!c.words("as", "long", "as")) return null;
      return parseCondition(c);
    });
    if (cond) condition = cond;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "cant", what, action, condition };
  });
  if (cant) return cant;

  // "<subject> attacks each combat if able."
  const mustAttack = c.attempt((c): StaticEffect | null => {
    const what = parseStaticSubject(c);
    if (!what) return null;
    if (!c.words("attacks", "each", "combat", "if", "able")) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "must-attack", what };
  });
  if (mustAttack) return mustAttack;

  // Anthem: "<subject> get(s)/have/gain(s) <modifiers> (as long as …)?"
  // A targeted subject or a one-shot duration ("until end of turn") means this
  // is resolution text, not a continuous ability — reject and let the spell
  // parser take it.
  const modify = c.attempt((c): StaticEffect | null => {
    const what = parseStaticSubject(c);
    if (!what || what.ref === "target") return null;
    const modifiers = parseStaticModifiers(c);
    if (!modifiers) return null;
    const duration = parseDuration(c);
    let condition: Condition | undefined;
    if (duration) {
      if (duration.duration !== "as-long-as") return null;
      condition = duration.condition;
    }
    c.punct(".");
    if (!c.done()) return null;
    return { static: "modify", what, modifiers, condition, duration: undefined };
  });
  if (modify) return modify;

  return c.fail("static ability");
}
