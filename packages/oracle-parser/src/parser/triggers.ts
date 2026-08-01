// Trigger condition parsing: the clause between "When/Whenever/At" and the
// comma that starts the effect text.

import type { ObjectRef, PhaseName, PlayerRef, Trigger, TurnOwner } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseFilter, parseObjectRef, parsePlayerRef } from "./refs.ts";

/** Subject of an event trigger: "~", "a creature", "another Elf you control" … */
function parseTriggerSubject(c: Cursor): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    if (c.word("a", "an") !== null) {
      const filter = parseFilter(c);
      if (!filter) return null;
      return { ref: "filter", filter, count: { amount: "fixed", value: 1 } };
    }
    if (c.attempt((c) => (c.words("one", "or", "more") ? true : null))) {
      const filter = parseFilter(c);
      if (!filter) return null;
      return { ref: "filter", filter, count: { amount: "fixed", value: 1 } };
    }
    // Bare filter subject ("another creature you control dies") is one object
    // per event, not "each" — so classify it as a filter ref, not via
    // parseObjectRef's bare-plural fallback.
    const bare = c.attempt(parseFilter);
    if (bare) return { ref: "filter", filter: bare, count: { amount: "fixed", value: 1 } };
    return parseObjectRef(c);
  });
}

function parsePhase(c: Cursor): PhaseName | null {
  if (c.words("upkeep")) return "upkeep";
  if (c.words("draw", "step")) return "draw-step";
  if (c.words("untap", "step")) return "untap-step";
  if (c.words("end", "step")) return "end-step";
  if (c.words("end", "of", "combat")) return "end-of-combat";
  if (c.words("combat", "damage", "step")) return "combat-damage";
  if (c.words("declare", "attackers", "step")) return "declare-attackers";
  if (c.words("declare", "blockers", "step")) return "declare-blockers";
  if (c.words("cleanup", "step")) return "cleanup";
  if (c.words("first", "main", "phase") || c.words("precombat", "main", "phase"))
    return "precombat-main";
  if (c.words("second", "main", "phase") || c.words("postcombat", "main", "phase"))
    return "postcombat-main";
  if (c.words("main", "phase")) return "main";
  if (c.words("combat")) return "combat";
  if (c.words("next", "turn") || c.words("turn")) return "turn";
  return c.fail("phase");
}

/** "At the beginning of <owner> <phase>" / "of <phase> on <owner> turn" */
function parsePhaseTrigger(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    if (!c.words("the", "beginning", "of")) return null;

    // "of your upkeep" / "of each player's upkeep" / "of each opponent's upkeep"
    let whose: TurnOwner | undefined;
    if (c.word("your") !== null) whose = "your";
    else if (c.attempt((c) => (c.word("each") !== null && c.possessive("player") !== null ? true : null)))
      whose = "each-player";
    else if (c.attempt((c) => (c.word("each") !== null && c.possessive("opponent") !== null ? true : null)))
      whose = "each-opponent";
    else if (c.attempt((c) => (c.word("that") !== null && c.possessive("player") !== null ? true : null)))
      whose = "that-player";
    else if (c.attempt((c) => (c.word("an") !== null && c.possessive("opponent") !== null ? true : null)))
      whose = "opponent";
    else if (c.word("each") !== null) whose = "each-player"; // "each upkeep", "each end step"
    else c.word("the");

    const phase = parsePhase(c);
    if (!phase) return null;

    // "… on your turn" / "on each player's turn"
    if (!whose) {
      if (c.attempt((c) => (c.words("on", "your", "turn") ? true : null))) whose = "your";
      else if (
        c.attempt((c) =>
          c.word("on") !== null && c.word("each") !== null && c.possessive("player") !== null && c.word("turn") !== null
            ? true
            : null,
        )
      )
        whose = "each-player";
    }
    return { trigger: "phase", phase, whose: whose ?? "each-player" };
  });
}

export function parseTrigger(c: Cursor): Trigger | null {
  const at = c.isWord("at");
  const opener = c.word("when", "whenever", "at");
  if (opener === null) return null;
  if (at || opener === "at") return parsePhaseTrigger(c);

  // "Whenever you cast a(n) <filter> spell"
  const youCast = c.attempt((c): Trigger | null => {
    let who: PlayerRef | null = null;
    if (c.word("you") !== null) who = { player: "you" };
    else {
      who = parsePlayerRef(c);
      if (!who) {
        if (c.attempt((c) => (c.word("a") !== null && c.word("player") !== null ? true : null)))
          who = { player: "each-player" };
        else if (c.attempt((c) => (c.word("an") !== null && c.word("opponent") !== null ? true : null)))
          who = { player: "an-opponent" };
      }
    }
    if (!who) return null;
    if (c.word("cast", "casts") === null) return null;
    c.word("a", "an");
    const what = parseFilter(c);
    if (!what) return null;
    return { trigger: "cast", who, what };
  });
  if (youCast) return youCast;

  // "Whenever you activate an ability …"
  const activate = c.attempt((c): Trigger | null => {
    let who: PlayerRef | null = null;
    if (c.word("you") !== null) who = { player: "you" };
    else {
      const p = parsePlayerRef(c);
      if (p) who = p;
    }
    if (!who) return null;
    if (c.word("activate", "activates") === null) return null;
    c.word("a", "an");
    const what = parseFilter(c);
    if (!what) return null;
    return { trigger: "activate", who, what };
  });
  if (activate) return activate;

  // "Whenever you attack"
  if (c.attempt((c) => (c.words("you", "attack") ? true : null))) return { trigger: "you-attack" };

  // Player life/draw/discard triggers.
  const playerEvent = c.attempt((c): Trigger | null => {
    const who = parsePlayerRef(c);
    if (!who) return null;
    if (c.word("gain", "gains") !== null && c.word("life") !== null)
      return { trigger: "gains-life", who };
    if (c.word("lose", "loses") !== null && c.word("life") !== null)
      return { trigger: "loses-life", who };
    if (c.word("draw", "draws") !== null && c.word("a") !== null && c.word("card") !== null)
      return { trigger: "draws", who };
    if (c.word("discard", "discards") !== null && c.word("a") !== null && c.word("card") !== null)
      return { trigger: "discards", who };
    if (c.word("sacrifice", "sacrifices") !== null) {
      c.word("a", "an");
      const what = parseFilter(c);
      if (!what) return null;
      return { trigger: "sacrifices", who, what };
    }
    if (c.word("scry", "scries") !== null) return { trigger: "scries", who };
    if (c.word("surveil", "surveils") !== null) return { trigger: "surveils", who };
    return null;
  });
  if (playerEvent) return playerEvent;

  // Object-subject triggers.
  const subject = parseTriggerSubject(c);
  if (!subject) return c.fail("trigger subject");

  // "enters (the battlefield)?" — possibly "or leaves" / "or attacks" / "or dies".
  // Plural verb forms cover collective subjects ("one or more creatures enter").
  if (c.word("enters", "enter") !== null) {
    c.words("the", "battlefield");
    // "enters the battlefield under your control"
    const underYourControl = c.attempt((c) => (c.words("under", "your", "control") ? true : null));
    if (underYourControl && (subject.ref === "filter" || subject.ref === "each")) {
      subject.filter.control = "you";
    }
    if (c.word("or") !== null) {
      if (c.word("leaves", "leave") !== null) {
        c.words("the", "battlefield");
        return { trigger: "enters-or-leaves", what: subject };
      }
      if (c.word("attacks", "attack") !== null) return { trigger: "enters-or-attacks", what: subject };
      if (c.word("dies", "die") !== null) return { trigger: "enters-or-dies", what: subject };
      return null;
    }
    return { trigger: "enters", what: subject };
  }
  if (c.word("leaves", "leave") !== null) {
    c.words("the", "battlefield");
    return { trigger: "leaves", what: subject };
  }
  if (c.word("dies", "die") !== null) return { trigger: "dies", what: subject };
  if (c.attempt((c) => (c.words("is", "put", "into") ? true : null))) {
    const your = c.word("a", "your");
    if (c.word("graveyard") === null) return null;
    if (!c.words("from", "the", "battlefield")) {
      c.words("from", "anywhere");
    }
    void your;
    return { trigger: "put-into-graveyard", what: subject, from: "battlefield" };
  }
  if (c.word("attacks", "attack") !== null) {
    const alone = c.word("alone") !== null;
    // optional "you or a planeswalker you control" tail
    const whom: (PlayerRef | ObjectRef)[] = [];
    const first = c.attempt(parsePlayerRef) ?? c.attempt(parseObjectRef);
    if (first) {
      whom.push(first);
      while (c.word("or") !== null) {
        const nxt = c.attempt(parsePlayerRef) ?? c.attempt(parseObjectRef);
        if (!nxt) return null;
        whom.push(nxt);
      }
    }
    return { trigger: "attacks", what: subject, alone: alone || undefined, whom: whom.length ? whom : undefined };
  }
  if (c.word("blocks") !== null) {
    if (c.attempt((c) => (c.words("or", "becomes", "blocked") ? true : null)))
      return { trigger: "blocks", what: subject, orBecomesBlocked: true };
    c.attempt((c) => {
      c.word("a", "an");
      return parseFilter(c);
    });
    return { trigger: "blocks", what: subject };
  }
  if (c.words("becomes", "blocked")) {
    const by = c.attempt((c): ObjectRef | null => {
      if (c.word("by") === null) return null;
      c.word("a", "an");
      const filter = parseFilter(c);
      return filter && { ref: "filter", filter, count: { amount: "fixed", value: 1 } };
    });
    return { trigger: "becomes-blocked", what: subject, by: by ?? undefined };
  }
  if (c.words("becomes", "tapped")) return { trigger: "becomes-tapped", what: subject };
  if (c.words("becomes", "untapped")) return { trigger: "becomes-untapped", what: subject };
  if (c.attempt((c) => (c.words("becomes", "the", "target", "of") ? true : null))) {
    c.word("a", "an");
    const of = parseFilter(c);
    if (!of) return null;
    return { trigger: "becomes-target", what: subject, of };
  }
  if (c.attempt((c) => (c.words("is", "dealt", "damage") ? true : null)))
    return { trigger: "is-dealt-damage", what: subject };
  if (c.word("deals") !== null) {
    const combat = c.word("combat") !== null;
    if (c.word("damage") === null) return null;
    let to: "any" | PlayerRef | ObjectRef | undefined;
    if (c.word("to") !== null) {
      if (c.attempt((c) => (c.words("a", "player") ? true : null))) to = "any";
      else {
        const p = c.attempt(parsePlayerRef);
        if (p) to = p;
        else {
          const o = parseObjectRef(c);
          if (!o) return null;
          to = o;
        }
      }
    }
    return { trigger: "deals-damage", what: subject, combat: combat || undefined, to };
  }
  if (c.attempt((c) => (c.words("is", "tapped", "for", "mana") ? true : null)))
    return { trigger: "taps-for-mana", what: subject };

  return c.fail("trigger event");
}
