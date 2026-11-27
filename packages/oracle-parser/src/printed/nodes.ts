import type { Ability } from "../ast.ts";
import type { Occurrence, PrintedCapture } from "./capture.ts";
import type { ImplicitReason, PrintedNode, TokenRun } from "./schema.ts";

export interface CaptureDefects {
  nesting: number;
  overlap: number;
  conflict: number;
  unlocatedSelections: number;
  mutationGuardDemotions: number;
}

export interface LocatedNode {
  value: Record<string, unknown>;
  node: PrintedNode;
  scope: string;
  depth: number;
  children: LocatedNode[];
}

export function tokenRuns(tokens: readonly Occurrence[]): TokenRun[] {
  const runs: TokenRun[] = [];
  const ordered = tokens.slice().sort((a, b) => a.line - b.line || a.index - b.index);
  for (const { line, index } of ordered) {
    const last = runs.at(-1);
    if (last?.[0] === line && index <= last[2]) last[2] = Math.max(last[2], index + 1);
    else runs.push([line, index, index + 1]);
  }
  return runs;
}

export function containsToken(runs: readonly TokenRun[], line: number, index: number): boolean {
  return runs.some(([l, start, end]) => l === line && start <= index && index < end);
}

function union(runs: readonly TokenRun[]): TokenRun[] {
  const result: TokenRun[] = [];
  for (const [line, start, end] of runs.slice().sort((a, b) => a[0] - b[0] || a[1] - b[1])) {
    const last = result.at(-1);
    if (last?.[0] === line && last[2] >= start) last[2] = Math.max(last[2], end);
    else result.push([line, start, end]);
  }
  return result;
}

export function extent(node: PrintedNode): TokenRun[] {
  if (node.basis === "recorded") return node.folds ? union([...node.span, ...node.folds]) : node.span;
  return node.basis === "hull" ? node.span : [];
}

function overlaps(a: TokenRun, b: TokenRun): boolean {
  return a[0] === b[0] && a[1] < b[2] && b[1] < a[2];
}

function pointer(path: string, key: string): string {
  return path + "/" + key.replaceAll("~", "~0").replaceAll("/", "~1");
}

const abilityKinds = new Set<Ability["kind"]>(["chapter", "max-speed", "infinity", "visit", "sticker", "exert-on-attack",
  "linked-triggers", "class-level-bar", "case-solve", "case-solved", "anchor-words", "keywords", "activated",
  "loyalty", "triggered", "static", "spell", "additional-cost"]);

export function locateNodes(ability: object, capture: PrintedCapture, lines: [number, number],
  defects: CaptureDefects): LocatedNode[] {
  const ordered: LocatedNode[] = [];
  const byPath = new Map<string, LocatedNode>();
  const fallbacks = new Map<LocatedNode, ImplicitReason>();
  const demote = (node: LocatedNode): void => {
    node.node = { path: node.node.path, basis: "unlocated" };
    for (const child of node.children) demote(child);
  };

  const targets = new Map<object, { path: string; value: object }>();
  // An alias re-originated in place keeps its identity for earlier aliases of it.
  const chains = new Map<object, object>();
  function findAliasTargets(value: unknown, path: string): void {
    if (!value || typeof value !== "object") return;
    const origin = capture.origin(value);
    const identity = capture.identity(value);
    if (origin?.basis === "alias") {
      if (identity && identity !== origin.target) chains.set(identity, origin.target);
      return;
    }
    if (identity && !targets.has(identity)) targets.set(identity, { path, value });
    for (const [key, child] of Object.entries(value)) findAliasTargets(child, pointer(path, key));
  }
  findAliasTargets(ability, "");

  function visit(value: unknown, path: string, scope: string, depth: number, parent?: LocatedNode,
    aliasOf?: string): void {
    if (!value || typeof value !== "object") return;
    if (Array.isArray(value)) {
      value.forEach((item, i) => visit(item, pointer(path, String(i)), scope, depth, parent,
        aliasOf === undefined ? undefined : pointer(aliasOf, String(i))));
      return;
    }
    const obj = value as Record<string, unknown>;
    if (abilityKinds.has(obj.kind as Ability["kind"])) scope = path;
    const origin = capture.origin(value);
    const mutated = origin?.basis === "recorded" && capture.changedSinceMark(value);
    if (mutated) {
      defects.mutationGuardDemotions++;
      aliasOf = undefined;
    }
    if (aliasOf === undefined && origin?.basis === "alias") {
      let identity = origin.target;
      for (const seen = new Set<object>(); !targets.has(identity) && chains.has(identity) && !seen.has(identity);) {
        seen.add(identity);
        identity = chains.get(identity)!;
      }
      const target = targets.get(identity);
      if (target && JSON.stringify(target.value) === JSON.stringify(value)) aliasOf = target.path;
    }

    const located: LocatedNode = { value: obj, node: { path, basis: "unlocated" }, scope, depth, children: [] };
    ordered.push(located);
    byPath.set(path, located);
    parent?.children.push(located);
    for (const [key, child] of Object.entries(value)) {
      const newBody = key === "effects" && (obj.effect === "create-reflexive-trigger" ||
        obj.effect === "create-delayed-trigger" && obj.targets === "delayed-ability");
      visit(child, pointer(path, key), newBody ? path : scope, depth + 1, located,
        aliasOf === undefined ? undefined : pointer(aliasOf, key));
    }

    if (aliasOf !== undefined) {
      located.node = { path, basis: "alias", of: aliasOf };
      if (origin?.basis === "alias" && origin.fallback) fallbacks.set(located, origin.fallback);
      return;
    }
    if (origin?.basis === "implicit" || origin?.basis === "alias" && origin.fallback) {
      located.node = { path, basis: "implicit", reason: origin.basis === "implicit" ? origin.reason : origin.fallback! };
      return;
    }

    const unclear = new Set<LocatedNode>();
    for (const [index, child] of located.children.entries()) {
      const spans = extent(child.node);
      for (const other of located.children.slice(0, index)) {
        if (spans.some(a => extent(other.node).some(b => overlaps(a, b)))) {
          defects.overlap++;
          unclear.add(child).add(other);
        }
      }
    }
    for (const child of unclear) demote(child);
    const childSpans = union(located.children.flatMap(child => extent(child.node)));
    if (mutated || origin?.basis === "hull") {
      const clauses = origin.basis === "hull" ? origin.tokens : origin.folds ?? [];
      // Rust recomputes a hull as its child extents plus these attributed
      // clause runs, so every folded clause is emitted with the hull.
      const clauseRuns = tokenRuns(clauses.filter(({ line }) => lines[0] <= line && line < lines[1]));
      const span = union([...childSpans, ...clauseRuns]);
      if (span.length) located.node = { path, basis: "hull", span, ...(clauseRuns.length ? { clauses: clauseRuns } : {}) };
      return;
    }
    if (origin?.basis === "recorded") {
      const span = tokenRuns(origin.tokens);
      const folds = origin.folds && tokenRuns(origin.folds);
      const coverage = folds ? union([...span, ...folds]) : span;
      if (coverage.some(([line]) => line < lines[0] || line >= lines[1]) ||
          folds?.some(fold => span.some(run => overlaps(run, fold)))) {
        defects.conflict++;
      } else if (childSpans.some(([line, start, end]) =>
        !coverage.some(([l, from, to]) => l === line && from <= start && end <= to))) {
        defects.nesting++;
      } else {
        // Rider clauses are structural containers including their spending
        // condition and body. The children retain the operative proofs.
        located.node = typeof obj.rider === "string" ? { path, basis: "hull", span: coverage, clauses: coverage }
          : { path, basis: "recorded", span, ...(folds ? { folds } : {}) };
        return;
      }
    }
    if (childSpans.length) located.node = { path, basis: "hull", span: childSpans };
  }

  visit(ability, "", "", 0);
  for (const located of ordered) {
    const node = located.node;
    if (node.basis === "alias") {
      const target = byPath.get(node.of)?.node;
      if (!target || target.basis === "alias" || target.basis === "unlocated") {
        const reason = fallbacks.get(located);
        located.node = reason ? { path: node.path, basis: "implicit", reason } : { path: node.path, basis: "unlocated" };
      }
    }
  }
  return ordered;
}
