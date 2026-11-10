// Voting (CR 701.38): the vote itself, and the clauses that read its result.
// A result clause is only meaningful after a vote that offered that choice
// in the same instruction sequence, so every reference is validated by
// `voteReferencesResolved` (called from parseSentences).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Condition, Effect, ObjectFilter, ObjectRef, PlayerFilter, PlayerRef, Sentence, VoteChoices } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import { parseFilter, parsePlayerRef } from "./refs.ts";

function isNonThenWord(token: Token | undefined): boolean {
  return token?.kind === "word" && token.value !== "then";
}

/** Named choices are words with no rules meaning, each tied to an effect
 * (CR 701.38b). Words run up to "or", a comma or the end of the list. */
function parseNames(c: Cursor): string[] | null {
  return c.attempt((c): string[] | null => {
    const names: string[] = [];
    for (;;) {
      const words: string[] = [];
      while (!c.isWord("or") && !c.isPunct(",") && !c.isPunct(".")) {
        const w = c.anyWord();
        if (!w) return null;
        words.push(w.raw);
      }
      if (!words.length) return null;
      names.push(words.join(" "));
      if (c.isPunct(",") && isNonThenWord(c.peek(1))) {
        c.punct(",");
        c.word("or");
        continue;
      }
      if (c.word("or") !== null) continue;
      break;
    }
    return names.length >= 2 ? names : null;
  });
}

function parseChoices(c: Cursor): VoteChoices | null {
  return c.attempt((c): VoteChoices | null => {
    if (c.words("a", "player")) return { choices: "players" };
    if (c.words("another", "player")) return { choices: "players", other: true };
    if (c.word("a", "an") !== null) {
      const filter: ObjectFilter | null = parseFilter(c);
      return filter ? { choices: "objects", filter } : null;
    }
    const names = parseNames(c);
    return names ? { choices: "named", names } : null;
  });
}

/** "[Starting with you,] each player [secretly] votes for A or B." */
export function parseVoteStep(c: Cursor): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    let starting: PlayerRef | undefined;
    if (c.words("starting", "with")) {
      const player = parsePlayerRef(c);
      if (!player || !c.punct(",")) return null;
      starting = player;
    }
    if (!c.words("each", "player")) return null;
    const secret = c.word("secretly") !== null;
    if (c.word("votes") === null || c.word("for") === null) return null;
    const choices = parseChoices(c);
    if (!choices) return null;
    if (secret && !(c.punct(",") && c.words("then", "those", "votes", "are", "revealed"))) return null;
    return [{ effect: "vote", ...(starting ? { starting } : {}), ...(secret ? { secret: true as const } : {}), choices }];
  });
}

/** "who voted for a choice you voted for / you didn't vote for": the voting
 * records of two players in the vote that just finished (CR 701.38a). The
 * reference player must be a single known player. */
export function parseVotedForQualifier(c: Cursor, g: { parsePlayerRef: (c: Cursor) => PlayerRef | null }): PlayerFilter | null {
  return c.attempt((c): PlayerFilter | null => {
    if (!c.words("who", "voted", "for", "a", "choice")) return null;
    const as = g.parsePlayerRef(c);
    if (!as || as.player !== "you") return null;
    if (c.words("voted", "for")) return { votedFor: { as, same: true } };
    return c.words("didn't", "vote", "for") ? { votedFor: { as, same: false } } : null;
  });
}

/** "A gets more votes [or the vote is tied]" / "A gets the most votes [or is
 * tied for the most votes]" — the clause after "If". */
export function parseVoteResultCondition(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    const words: string[] = [];
    while (!c.isWord("gets")) {
      const w = c.anyWord();
      if (!w) return null;
      words.push(w.raw);
    }
    c.word("gets");
    if (!words.length) return null;
    const choice = words.join(" ");
    if (c.words("more", "votes")) {
      return { condition: "vote-result", choice, result: c.words("or", "the", "vote", "is", "tied") ? "more-or-tied" : "more" };
    }
    if (c.words("the", "most", "votes")) {
      const tied = c.words("or", "is", "tied", "for", "the", "most", "votes");
      return { condition: "vote-result", choice, result: tied ? "most-or-tied" : "most" };
    }
    return null;
  });
}

/** "the number of A votes" — words up to "votes". */
export function parseVoteCountAmount(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    if (!c.words("the", "number", "of")) return null;
    const words: string[] = [];
    while (!c.isWord("votes")) {
      const w = c.anyWord();
      if (!w || words.length >= 4) return null;
      words.push(w.raw);
    }
    c.word("votes");
    return words.length ? { amount: "vote-count", choice: words.join(" ") } : null;
  });
}

/** "for each A vote" (after "for each" has been consumed): the choice name. */
export function parseVoteRiderChoice(c: Cursor): string | null {
  return c.attempt((c): string | null => {
    const words: string[] = [];
    while (!c.isWord("vote")) {
      const w = c.anyWord();
      if (!w || words.length >= 4) return null;
      words.push(w.raw);
    }
    c.word("vote");
    return words.length ? words.join(" ") : null;
  });
}

/** "each permanent with the most votes or tied for most votes". */
export function parseVoteLeaders(c: Cursor, filterOf: (c: Cursor) => ObjectFilter | null): ObjectRef | null {
  if (!c.isWord("each")) return null;
  return c.attempt((c): ObjectRef | null => {
    c.word("each");
    const tail = ["with", "the", "most", "votes", "or", "tied", "for", "most", "votes"];
    let end = -1;
    for (let i = c.pos; i < c.tokens.length; i++) {
      if (tail.every((w, k) => { const t = c.tokens[i + k]; return t?.kind === "word" && t.value === w; })) { end = i; break; }
    }
    if (end < 0) return null;
    const sub = new Cursor(c.tokens.slice(c.pos, end));
    const filter = filterOf(sub);
    if (!filter || !sub.done()) return null;
    c.pos = end + tail.length;
    return { ref: "vote-leaders", filter };
  });
}

const REPEATABLE = new Set(["sacrifice", "discard", "add-turns", "keyword-action"]);
const OBJECT_VOTE = "\u0000objects";

/** Names offered by every vote in `sentences`. */
function offeredNames(sentences: Sentence[]): Set<string> {
  const names = new Set<string>();
  const visit = (value: unknown): void => {
    if (Array.isArray(value)) { value.forEach(visit); return; }
    if (!value || typeof value !== "object") return;
    const node = value as Record<string, unknown>;
    if (node.effect === "vote") {
      const choices = node.choices as VoteChoices;
      if (choices.choices === "named") choices.names.forEach(name => names.add(name.toLowerCase()));
      else if (choices.choices === "objects") names.add(OBJECT_VOTE);
    }
    Object.values(node).forEach(visit);
  };
  visit(sentences);
  return names;
}

/** Every vote-result condition, vote count and per-vote repeat in `sentence`
 * must name a choice offered by an earlier vote in this sequence. */
export function voteReferencesResolved(previous: Sentence[], sentence: Sentence): boolean {
  const used: string[] = [];
  let valid = true;
  const visit = (value: unknown): void => {
    if (Array.isArray(value)) { value.forEach(visit); return; }
    if (!value || typeof value !== "object") return;
    const node = value as Record<string, unknown>;
    if (node.condition === "vote-result" || node.amount === "vote-count") used.push(String(node.choice));
    // Only instructions that simply repeat per vote are represented; scans
    // (a count of cards) and optional choices would change meaning.
    if (node.forEachVote && (!REPEATABLE.has(String(node.effect)) || node.optional)) valid = false;
    if (node.forEachVote && typeof node.forEachVote === "object") used.push(String((node.forEachVote as { choice: string }).choice));
    if (node.ref === "vote-leaders") used.push(OBJECT_VOTE);
    Object.values(node).forEach(visit);
  };
  visit(sentence);
  if (!valid) return false;
  if (!used.length) return true;
  const offered = offeredNames(previous);
  return used.every(choice => offered.has(choice.toLowerCase()));
}
