import type { Token } from "../lexer.ts";
import { wordNumber } from "../vocab.ts";
import type { CaptureDefects, LocatedNode } from "./nodes.ts";
import { containsToken, extent } from "./nodes.ts";
import type { Edit, PrintedFact, PrintedToken, TokenRun } from "./schema.ts";
import { EXCEPTIONS, INTRODUCERS, MENTIONS, POLARITIES } from "./vocabulary.ts";

export interface LexedOccurrence {
  token: Token;
  wire: PrintedToken;
  selectionIds: Set<string>;
}
type TokenFact = Exclude<PrintedFact, { fact: "reminder-citation" }>;
type StripLocation<T> = T extends TokenFact ? Omit<T, "at" | "tok" | "field"> : never;
type LexicalFact = StripLocation<TokenFact>;

function member<const T extends string>(values: readonly T[], value: string): value is T {
  return (values as readonly string[]).includes(value);
}

function fieldFor(fact: LexicalFact, path: string, value: Record<string, unknown>, word: string): string | null {
  if (fact.fact === "quantity" && value.amount === "fixed" && fact.value === value.value) return path + "/value";
  if (fact.fact === "optional") {
    const { form } = fact;
    if (form === "up-to" && value.upTo === true) return path + "/upTo";
    if (form !== "up-to" && value.optional === true) return path + "/optional";
  }
  if (fact.fact === "exception" && ["another", "other"].includes(fact.form)) {
    if (value.other === true) return path + "/other";
    if (typeof value.filter === "object" && value.filter !== null && "other" in value.filter && value.filter.other === true)
      return path + "/filter/other";
  }
  if (fact.fact === "polarity" && fact.form === "only" && value.restriction === "only")
    return path + "/restriction";
  if (fact.fact === "polarity" && fact.form === "can't" && value.effect === "cant") return path + "/effect";
  if (fact.fact === "polarity" && fact.form === "non-prefix") {
    const fields = ["nonTypes", "nonSubtypes", "nonSupertypes"].filter(key =>
      Array.isArray(value[key]) && value[key].some((trait: unknown) =>
        typeof trait === "string" && trait.toLowerCase() === word.slice(3)));
    if (fields.length === 1) return path + "/" + fields[0];
  }
  if (fact.fact === "mention" && fact.form === "this-way" && value.scope === "this-way") return path + "/scope";
  if (fact.fact === "mention" && (fact.form === "you" || fact.form === "your") && value.player === "you")
    return path + "/player";
  return null;
}

export function collectFacts(lines: readonly (readonly LexedOccurrence[])[], first: number, last: number,
  nodes: readonly LocatedNode[], edits: readonly Edit[], defects: CaptureDefects): TokenFact[] {
  const facts: TokenFact[] = [];
  const declarations = new Map<string, { line: number; index: number; origin: LocatedNode }>();
  const byId = new Map<string, { line: number; index: number }[]>();
  const scopes = new Map<string, number>();

  for (let line = first; line < last; line++) {
    lines[line].forEach(({ selectionIds }, index) => {
      for (const id of selectionIds) {
        const current = byId.get(id) ?? [];
        current.push({ line, index });
        byId.set(id, current);
      }
    });
  }
  const selections = new Set<string>();
  for (const origin of nodes) {
    const id = origin.value.selectionId;
    if (typeof id !== "string" || origin.node.basis === "alias") continue;
    const candidates = byId.get(id) ?? [];
    const inside = candidates.filter(({ line, index }) => containsToken(extent(origin.node), line, index));
    const matches = inside.length ? inside : candidates;
    const selection = origin.scope + "\0" + id;
    if (matches.length !== 1) {
      if (!selections.has(selection)) defects.unlocatedSelections++;
      selections.add(selection);
      continue;
    }
    const [match] = matches;
    const key = `${match.line}:${match.index}`;
    const previous = declarations.get(key);
    // Reused reference nodes in one scope do not create a second occurrence.
    // Prefer the actual spanned production over a displaced copy.
    if (!previous || inside.length && !containsToken(extent(previous.origin.node), match.line, match.index))
      declarations.set(key, { ...match, origin });
  }

  for (let line = first; line < last; line++) {
    const tokens = lines[line];
    const owners = tokens.map(() => nodes[0]);
    for (const candidate of nodes) {
      for (const [l, from, end] of extent(candidate.node)) {
        if (l !== line) continue;
        for (let index = from; index < end; index++) {
          if (candidate.depth >= owners[index].depth) owners[index] = candidate;
        }
      }
    }
    for (const [index, surface] of tokens.entries()) {
      const { token, wire } = surface;
      const owner = owners[index];
      const at = owner?.node.path ?? "";
      const word = token.kind === "word" ? token.value : "";
      const emit = (fact: LexicalFact, end = index + 1) => {
        // A multi-token observation crossing an ownership boundary belongs
        // to the nearest enclosing node containing the complete phrase.
        const phraseOwner = end === index + 1 ? owner : nodes.filter(candidate => {
          const coverage = extent(candidate.node);
          return coverage.some(([l, from, to]) => l === line && from <= index && end <= to);
        }).at(-1) ?? nodes[0];
        const path = phraseOwner?.node.path ?? at;
        const tok: TokenRun = [line, index, end];
        facts.push({ ...fact, at: path, tok, field: phraseOwner ? fieldFor(fact, path, phraseOwner.value, word) : null });
      };
      const next = tokens[index + 1]?.token;
      const nextWord = next?.kind === "word" ? next.value : "";
      const edit = edits.find(edit => (edit.edit === "self-name" || edit.edit === "pronoun") &&
        edit.n[0] < wire.n[1] && wire.n[0] < edit.n[1]);
      if (edit?.edit === "self-name" || edit?.edit === "pronoun")
        emit({ fact: "mention", form: edit.edit === "self-name" ? "source-name" : "gendered", ...("possessive" in token && token.possessive ? { possessive: true } : {}) });
      else if (member(MENTIONS, word))
        emit({ fact: "mention", form: word, ...(token.kind === "word" && token.possessive ? { possessive: true } : {}) });

      if (word === "target" || word === "targets") {
        const declaration = declarations.get(`${line}:${index}`);
        if (declaration) {
          const { scope, node, value } = declaration.origin;
          const targetIndex = scopes.get(scope) ?? 0;
          scopes.set(scope, targetIndex + 1);
          facts.push({ fact: "target", at, tok: [line, index, index + 1], targetIndex, scope,
            selectionId: value.selectionId as string, field: node.path + "/selectionId" });
          emit({ fact: "introducer", form: "target" });
        } else emit({ fact: "mention", form: "target", ...(token.kind === "word" && token.possessive ? { possessive: true } : {}) });
      }

      const quantity = token.kind === "number" ? token.value : wordNumber(word);
      if (typeof quantity === "number" && Number.isFinite(quantity)) {
        emit({ fact: "quantity", form: token.kind === "number" ? "digits" : "word", value: quantity });
        emit({ fact: "introducer", form: "numeral" });
      }
      if (word === "x") emit({ fact: "quantity", form: "x", value: null });
      if (word === "a" || word === "an") emit({ fact: "quantity", form: "article", value: 1 });
      const introducer = word === "an" ? "a" : word;
      if (member(INTRODUCERS, introducer)) emit({ fact: "introducer", form: introducer });

      if (word === "up" && nextWord === "to") {
        emit({ fact: "optional", form: "up-to" }, index + 2);
        emit({ fact: "introducer", form: "up-to" }, index + 2);
      }
      if (word === "may") emit({ fact: "optional", form: nextWord === "have" ? "may-have" : "may" });
      if (member(POLARITIES, word)) emit({ fact: "polarity", form: word });
      if (word.startsWith("non") && word.length > 3 && word !== "none")
        emit({ fact: "polarity", form: "non-prefix", n: [wire.n[0], wire.n[0] + 3] });
      if (member(EXCEPTIONS, word)) {
        if (word === "other" && nextWord === "than") emit({ fact: "exception", form: "other-than" }, index + 2);
        else emit({ fact: "exception", form: word });
      }
      if (word === "rather" && nextWord === "than") emit({ fact: "exception", form: "rather-than" }, index + 2);
      if (word === "this" && nextWord === "way") emit({ fact: "mention", form: "this-way" }, index + 2);
      if (word === "that" && nextWord === "many") emit({ fact: "mention", form: "that-many" }, index + 2);
      if (word === "the" && (nextWord === "chosen" || nextWord === "rest"))
        emit({ fact: "mention", form: nextWord === "chosen" ? "the-chosen" : "the-rest" }, index + 2);
      if (token.kind === "punct") {
        const role = { ".": "sentence-end", ",": "comma", ":": "colon", "—": "dash", "•": "bullet",
          '"': "quote", "'": "inner-quote" } as const;
        emit({ fact: "punct", role: token.value in role ? role[token.value as keyof typeof role] : "other",
          ...(wire.grammarIndex !== index ? { moved: true } : {}) });
      }
    }
  }
  return facts;
}
