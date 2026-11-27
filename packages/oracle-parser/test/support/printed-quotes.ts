import { containsToken, extent } from "../../src/printed/nodes.ts";
import type { PrintedBlock, PrintedFace, TokenRun } from "../../src/printed/schema.ts";

type ReadFace = Extract<PrintedFace, { status: "read" }>;
type ReadBlock = Extract<PrintedBlock, { status: "read" }>;
type Quote = { line: number; open: number; openOrder: number; close?: number; mark: string; parent?: Quote };
type Region = Quote | undefined;

const termClauses = [
  { term: "legend rule", words: ["the", '"', "legend", "rule", '"', "doesn't", "apply"], closeOffset: 3 },
  { term: "bands with other", words: ["all", '"', "bands", "with", "other", '"', "abilities"], closeOffset: 4 },
] as const;
type Term = (typeof termClauses)[number]["term"];

export function nodesAtPaths(value: unknown, path = "", nodes = new Map<string, Record<string, unknown>>()) {
  if (Array.isArray(value)) value.forEach((item, index) => nodesAtPaths(item, `${path}/${index}`, nodes));
  else if (value && typeof value === "object") {
    const object = value as Record<string, unknown>;
    nodes.set(path, object);
    for (const [key, child] of Object.entries(object))
      nodesAtPaths(child, path + "/" + key.replaceAll("~", "~0").replaceAll("/", "~1"), nodes);
  }
  return nodes;
}

/** Independent of capture. Rebuild regions from wire tokens in grammar order,
 * so punctuation moved out by the lexer belongs to the enclosing production. */
export function quoteChecks(face: ReadFace, normalized: string) {
  const bytes = Buffer.from(normalized);
  const lines = face.lines.map((line, lineIndex) => {
    const regions: Region[] = [];
    const pairs: Quote[] = [];
    const terms = new Map<Quote, { term: Term; indices: number[] }>();
    if (line.status !== "lexed") return { regions, pairs, terms };
    const tokens = line.tokens.map((token, index) => ({
      ...token, text: bytes.toString("utf8", ...token.n), index,
    })).sort((a, b) => a.grammarIndex - b.grammarIndex);
    if (!tokens.some(({ text, t }) => t === "punct" && text === '"')) return { regions, pairs, terms };
    const stack: Quote[] = [];
    for (const { index, t, text, grammarIndex } of tokens) {
      const mark = t === "punct" && (text === '"' || text === "'");
      const previous = mark ? stack.map(quote => quote.mark).lastIndexOf(text) : -1;
      if (previous >= 0) {
        const pair = stack[previous];
        pair.close = index;
        stack.length = previous;
        const shape = termClauses.find(clause => grammarIndex === pair.openOrder + clause.closeOffset);
        const start = pair.openOrder - 1;
        if (shape && start >= 0) {
          const clause = tokens.slice(start, start + shape.words.length);
          if (clause.length === shape.words.length && clause.every((token, offset) => {
            const word = shape.words[offset];
            if (word === '"') return token.t === "punct" && token.text === pair.mark;
            return token.t === "word" && !token.possessive && token.text.toLowerCase() === word;
          })) terms.set(pair, { term: shape.term, indices: clause.map(token => token.index) });
        }
      }
      const parent = stack.at(-1);
      regions[index] = parent;
      if (mark && previous < 0) {
        const quote = { line: lineIndex, open: index, openOrder: grammarIndex, mark: text, parent };
        pairs.push(quote);
        stack.push(quote);
      }
    }
    return { regions, pairs, terms };
  });

  return (block: ReadBlock, ability: unknown): string[] => {
    const pairs = lines.slice(...block.lines).flatMap(line => line.pairs);
    if (!pairs.length) return [];
    const values = nodesAtPaths(ability);
    const byPath = new Map(block.nodes.map(node => [node.path, node]));
    const grants = new Map<Quote, string>();
    const operands = new Map<Quote, string>();
    const scopes = new Map<string, Region | null>([["", undefined]]);
    const errors: string[] = [];
    const owns = (path: string, owner: string) => path === "" || path === owner || owner.startsWith(path + "/");
    const covers = (runs: readonly TokenRun[], quote: Quote) => quote.close !== undefined &&
      containsToken(runs, quote.line, quote.open) && containsToken(runs, quote.line, quote.close);
    const immediate = (path: string): string => {
      let parent = path.slice(0, path.lastIndexOf("/"));
      while (parent && !byPath.has(parent)) parent = parent.slice(0, parent.lastIndexOf("/"));
      return parent;
    };
    const located = block.nodes.flatMap(node => {
      const runs = extent(node);
      return runs.length ? [{ path: node.path, runs, depth: node.path.split("/").length }] : [];
    });
    for (const node of block.nodes) {
      const { path } = node;
      if (!scopes.has(path)) scopes.set(path, scopes.get(immediate(path)));
      const region = scopes.get(path);
      const runs = extent(node);
      const value = values.get(path);
      if (value?.gained === "quoted") {
        const matching = pairs.filter(pair => pair.parent === region && covers(runs, pair) && !lines[pair.line].terms.has(pair));
        const [pair] = matching;
        if (runs.length && (matching.length !== 1 || grants.has(pair))) errors.push(`invalid quote enclosure at ${path}`);
        scopes.set(path + "/ability", pair ?? null);
        if (pair) {
          grants.set(pair, path);
          const body = byPath.get(path + "/ability");
          if (!body || !extent(body).length) errors.push(`unlocated quoted ability at ${path}/ability`);
        }
      }
      for (const pair of pairs) {
        if (pair.parent !== region || !covers(runs, pair)) continue;
        const clause = lines[pair.line].terms.get(pair);
        if (!clause || !clause.indices.every(index => containsToken(runs, pair.line, index))) continue;
        const parent = values.get(immediate(path));
        const [keyword] = Array.isArray(value?.abilities) && value.abilities.length === 1 ? value.abilities : [];
        if (clause.term === "legend rule" && (value?.static === "legend-rule-exemption" || value?.effect === "legend-rule-exemption") ||
          clause.term === "bands with other" && path.endsWith("/removal") && value?.removal === "listed" &&
          (parent?.modifier === "lose-abilities" || parent?.effect === "lose-abilities") &&
          keyword?.gained === "keyword" && keyword.keyword?.keyword === clause.term && Object.keys(keyword.keyword).length === 1) {
          if (operands.has(pair)) errors.push(`quoted term reuses its enclosure at ${path}`);
          operands.set(pair, path);
        }
      }
    }

    const checkRuns = (runs: readonly TokenRun[], path: string, field: string) => {
      for (const [pair, owner] of operands) {
        // The clause has no movable punctuation. Any overlap with the term
        // must include its entire physical token interval, including both marks.
        const interval = Array.from({ length: pair.close! - pair.open + 1 }, (_, offset) => pair.open + offset);
        const covered = interval.filter(index => containsToken(runs, pair.line, index));
        if (covered.length && (!owns(path, owner) || covered.length !== interval.length))
          errors.push(`${field} at ${path} crosses an unowned or split quoted term`);
      }
      for (const [line, start, end] of runs) {
        for (let index = start; index < end; index++) {
          let actual = lines[line].regions[index];
          while (actual !== scopes.get(path)) {
            const owner = actual && (grants.get(actual) ?? operands.get(actual));
            if (!actual || owner === undefined || !owns(path, owner) || !covers(runs, actual)) {
              errors.push(`${field} at ${path} crosses or escapes its quote region at ${line}:${index}`);
              break;
            }
            actual = actual.parent;
          }
        }
      }
    };
    for (const node of block.nodes) {
      if (node.basis === "recorded" || node.basis === "hull") {
        checkRuns(node.span, node.path, "span");
        checkRuns((node.basis === "hull" ? node.clauses : node.folds) ?? [], node.path, node.basis === "hull" ? "clauses" : "folds");
      } else if (node.basis === "alias" && scopes.get(node.path) !== scopes.get(node.of)) {
        errors.push(`alias at ${node.path} escapes its quote region`);
      }
    }
    for (const pair of pairs) {
      if (pair.close !== undefined && !grants.has(pair) && !operands.has(pair))
        errors.push(`unowned quote pair at ${pair.line}:${pair.open}`);
    }
    for (const [index, fact] of block.facts.entries()) {
      if (fact.fact === "reminder-citation") {
        if (pairs.some(pair => {
          const line = face.lines[pair.line];
          return line.status === "lexed" && pair.close !== undefined &&
            line.tokens[pair.open].o[1] < fact.o[1] && fact.o[0] < line.tokens[pair.close].o[0];
        })) errors.push(`reminder citation ${index} intersects a quote region`);
        continue;
      }
      const [line, start, end] = fact.tok;
      const region = lines[line].regions[start];
      const termOwner = region && operands.get(region);
      const scope = termOwner === fact.at ? region?.parent : region;
      let owner = "";
      let depth = 0;
      for (const node of located) {
        if (node.depth >= depth && node.runs.some(([l, from, to]) => line === l && from <= start && end <= to)) {
          owner = node.path;
          depth = node.depth;
        }
      }
      if (fact.at !== owner) errors.push(`fact ${index} at ${fact.at} does not name the innermost containing node ${owner}`);
      if (!byPath.has(fact.at) || scope !== scopes.get(fact.at) || lines[line].regions.slice(start, end).some(other => other !== region))
        errors.push(`fact ${index} at ${fact.at} crosses or escapes its quote region`);
    }
    return errors;
  };
}
