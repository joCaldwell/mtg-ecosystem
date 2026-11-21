// The authoritative resolution contract (archive R4): for every parsed ability
// block, each reference resolved to the closed `Ref` vocabulary shared with
// the Rust kernel IR, each printed step's obligation, and the span
// certificate. Paths and ids are relative to the block (`/effects/0/...`),
// never card-global. The diagnostic reference graph is the evidence these are
// derived from; anything it binds outside this vocabulary, or leaves
// unresolved, is a named residual and the block is incomplete. Wrong binding
// is worse than an unresolved reference: nothing is guessed here.
// CR 109.5, 113.7a, 201.5, 301.5f, 303.4m, 400.7, 601.2c, 603.2, 607.2a,
// 608.2c, 608.2h, 614.12, 107.3a/c/m:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ParsedLine } from "../ast.ts";
import { blockSpans, type SpanCertificate } from "./spans.ts";
import { STRUCTURAL_STEPS, isStructuralStep, type InstructionStep, type StepEffect, type StructuralScope } from "./step-vocabulary.ts";
import type { BindingNode, CardBindingAnalysis, CardBindingInput } from "./types.ts";

/** Who "you" (or "its owner" on a card in a hidden zone) denotes. */
export type ControllerRole =
  /** CR 109.5: the controller of the resolving spell. */
  | "spell-controller"
  /** CR 109.5, 602.2, 603.3a: the controller of an activated or triggered ability. */
  | "ability-controller"
  /** CR 109.5, 611.2: the controller of the source of a static ability. */
  | "static-controller"
  /** CR 614.12: the player under whose control the permanent will enter. */
  | "entering-controller"
  /** CR 601.2, 601.3: the player who would cast this card. */
  | "prospective-caster"
  /** CR 116.2: the player taking a special action with this card. */
  | "special-action-player"
  /** CR 108.3: the owner of this card outside the battlefield and stack. */
  | "card-owner"
  /** CR 113.10: the player who has a granted ability. */
  | "ability-holder";

/** What a printed step produced that a later reference denotes (CR 608.2c, 608.2h). */
export type StepResultRole = "objects" | "player" | "quantity" | "selection" | "remainder" | "choice";

/** The closed reference vocabulary (one-to-one with the kernel IR `Ref`). */
export type ResolvedRef =
  /** CR 113.7a, 201.5, 400.7: the ability's source object, this incarnation. */
  | { ref: "source" }
  | { ref: "controller"; role: ControllerRole }
  /** CR 601.2c: the block's `slot`-th announced target (printed order). */
  | { ref: "target"; slot: number }
  /** CR 301.5f, 303.4m: what the source Aura or Equipment is attached to. */
  | { ref: "attached-to" }
  /** CR 603.2: an object of the triggering event; `role` is the trigger's AST discriminant. */
  | { ref: "event-object"; role: string }
  /** CR 603.2: a player of the triggering event; `role` is the trigger's AST discriminant. */
  | { ref: "event-player"; role: string }
  /** CR 614.12: the permanent entering the battlefield, distinct from the source. */
  | { ref: "entering-object" }
  /** CR 608.2c, 608.2h: a result of the block's earlier step at `step`. */
  | { ref: "step-result"; step: string; role: StepResultRole }
  /** CR 107.3c, 608.2c: the value or member bound at `id` ("where X is", "for each"). */
  | { ref: "bound-var"; id: string }
  /** CR 607.2a: the cards exiled by a linked ability pair. */
  | { ref: "linked-pool"; id: string }
  /** CR 107.3a, 107.3m: the value announced for X. */
  | { ref: "announced-x" };

/** A player derived from a resolved reference ("its owner", "each opponent"). */
export type PlayerRelation = "owner" | "controller" | "opponents" | "other-players" | "defending";

/** One reference obligation at a block-relative AST path. `scope` names the
 * nested ability (block-relative) whose source, controller or targets the
 * reference uses; it is omitted for the block itself. */
export type RefEntry = { path: string; scope?: string } & (
  | { ref: ResolvedRef }
  | { derived: { relation: PlayerRelation; of: ResolvedRef } }
  /** A set, filter or compound operand evaluated where it is printed; its
   * own operands carry their own entries (CR 608.2h). */
  | { evaluated: "selection" }
  | { unresolved: string });

/** What each printed step must be accounted for by: its own event, or the
 * scope a structural step opens (semantic/step-vocabulary.ts). */
export type StepEntry = { path: string; astEffect: StepEffect } & (
  | { event: InstructionStep }
  | { scope: StructuralScope }
  | { unaccounted: true });

export interface BlockResolution {
  refs: RefEntry[];
  steps: StepEntry[];
  spans: { certificate: SpanCertificate } | { unsupported: string };
  /** Engine capabilities the block needs beyond reference binding. */
  engineFeatures: string[];
  /** Exact reasons the block is not completely resolved (empty when it is). */
  residuals: string[];
}
export interface FaceResolution { sourceFaceIndex: number; blocks: BlockResolution[] | null }
export interface CardResolution {
  artifactKind: "oracle-resolution";
  schemaVersion: 1;
  faces: FaceResolution[];
  /** Card-level reasons outside any block (an unparsed face, an invalid source). */
  residuals: string[];
  complete: boolean;
}

const CONTROLLER_ROLES: Record<string, ControllerRole | undefined> = {
  "spell-controller": "spell-controller", "activator": "ability-controller", "trigger-controller": "ability-controller",
  "static-source-controller": "static-controller", "entering-controller": "entering-controller",
  "prospective-caster": "prospective-caster", "special-action-player": "special-action-player", "card-owner": "card-owner",
};
const RELATIONS = new Set<string>(["owner", "controller", "opponents", "other-players", "defending"]);
const LINKED_POOLS = new Set<string>(["linked-exile-pool", "hideaway-exiled-pool", "cascade-exiled-pool"]);

/** The path of the step that owns `path` (`/effects/0/steps/1/what` → `/effects/0/steps/1`). */
function owningStep(path: string): string | null {
  const match = /^(.*\/steps\/\d+)(?:\/|$)/.exec(path);
  return match ? match[1] : null;
}

function collectSteps(value: unknown, path: string, out: { path: string; effect: StepEffect }[]): void {
  if (Array.isArray(value)) { value.forEach((item, index) => collectSteps(item, `${path}/${index}`, out)); return; }
  if (!value || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    const childPath = `${path}/${key.replace(/~/g, "~0").replace(/\//g, "~1")}`;
    if (key === "steps" && Array.isArray(child)) child.forEach((step, index) => {
      if (step && typeof step === "object" && typeof (step as { effect?: unknown }).effect === "string")
        out.push({ path: `${childPath}/${index}`, effect: (step as { effect: StepEffect }).effect });
    });
    collectSteps(child, childPath, out);
  }
}

export function resolveCard(input: CardBindingInput, analysis: CardBindingAnalysis): CardResolution {
  const prefix = `card:${input.identity}`;
  const local = (id: string) => id.startsWith(prefix) ? id.slice(prefix.length) : id;
  const nodes = new Map<string, BindingNode>(analysis.nodes.map(node => [local(node.id), node]));
  const cardResiduals = new Set<string>();
  const faces: FaceResolution[] = input.faces.map(face => {
    const result = face.result;
    if (!result?.ok) { cardResiduals.add("face:unparsed"); return { sourceFaceIndex: face.sourceFaceIndex, blocks: null }; }
    const blocks = result.abilities.map((ability, index) =>
      resolveBlock(`/faces/${face.sourceFaceIndex}/abilities/${index}`, ability, result.lines[index], nodes, analysis, local));
    return { sourceFaceIndex: face.sourceFaceIndex, blocks };
  });
  const roots = faces.flatMap(face => (face.blocks ?? []).map((_, index) => `/faces/${face.sourceFaceIndex}/abilities/${index}`));
  const within = (path: string) => roots.some(root => path === root || path.startsWith(`${root}/`));
  for (const diagnostic of analysis.diagnostics) if (!within(diagnostic.path)) cardResiduals.add(`diagnostic:${diagnostic.code}`);
  for (const reference of analysis.references) if (!within(reference.path)) cardResiduals.add("ref:outside-any-block");
  const residuals = [...cardResiduals].sort();
  return {
    artifactKind: "oracle-resolution", schemaVersion: 1, faces, residuals,
    complete: !residuals.length && faces.every(face => face.blocks?.every(block => !block.residuals.length)),
  };
}

function resolveBlock(root: string, ability: unknown, line: ParsedLine | undefined, nodes: Map<string, BindingNode>,
  analysis: CardBindingAnalysis, local: (id: string) => string): BlockResolution {
  const inBlock = (path: string) => path === root || path.startsWith(`${root}/`);
  const rel = (path: string) => path.slice(root.length);
  const residuals = new Set<string>();
  const blockScope = `${root}#ability`;
  const scopeOf = (scope: string | undefined): string | undefined => {
    if (scope === undefined) return undefined;
    const id = local(scope);
    if (id === blockScope) return undefined;
    const node = nodes.get(id);
    return node && inBlock(node.path) ? rel(node.path) : `!${id}`;
  };
  // CR 601.2c: targets are numbered per ability scope, in announcement order.
  const slots = new Map<string, number>();
  const counters = new Map<string, number>();
  for (const [id, node] of nodes) {
    if (node.node !== "selection" || node.selection !== "target" || !inBlock(node.path)) continue;
    const scope = local(node.scope);
    const slot = counters.get(scope) ?? 0;
    counters.set(scope, slot + 1);
    slots.set(id, slot);
  }
  // CR 603.2: the trigger events of this block (and of nested triggered
  // abilities), by their path; results under one are that event's participants.
  const triggers = new Map<string, string>();
  for (const node of nodes.values())
    if (node.node === "event" && inBlock(node.path) && node.operation.startsWith("trigger:")) triggers.set(node.path, node.operation.slice("trigger:".length));
  const triggerOf = (path: string): string | undefined => {
    for (const [at, role] of triggers) if (path === at || path.startsWith(`${at}/`)) return role;
    return undefined;
  };
  const eventRole = (event: string): { path: string; role: string } | undefined => {
    const node = nodes.get(local(event));
    return node?.node === "event" && node.operation.startsWith("trigger:") ? { path: node.path, role: node.operation.slice("trigger:".length) } : undefined;
  };

  type Resolved = { ref: ResolvedRef; scope?: string } | { derived: { relation: PlayerRelation; of: ResolvedRef }; scope?: string }
    | { evaluated: "selection" } | { unresolved: string };
  const resolve = (id: string, at: string, depth = 0): Resolved => {
    const node = nodes.get(id);
    if (!node) return { unresolved: "missing-node" };
    const name = `${node.node}${"role" in node && typeof node.role === "string" ? `:${node.role}` : ""}`;
    if (node.node === "source") return { ref: { ref: "source" } };
    if (!inBlock(node.path)) return { unresolved: `cross-block:${name}` };
    const scope = scopeOf("scope" in node ? node.scope as string : undefined);
    if (scope?.startsWith("!")) return { unresolved: `foreign-scope:${name}` };
    const scoped = <T extends object>(value: T) => scope === undefined ? value : { ...value, scope };
    const result = ((): Resolved => { switch (node.node) {
      case "runtime-player": {
        const role = CONTROLLER_ROLES[node.role];
        if (role) return scoped({ ref: { ref: "controller", role } as ResolvedRef });
        // CR 702.21a: ward's payer is the controller of the spell or ability
        // whose targeting triggered it.
        if (node.role === "targeting-player") return scoped({ ref: { ref: "event-player", role: "becomes-target" } as ResolvedRef });
        return { unresolved: name };
      }
      case "selection": {
        if (node.selection === "target") return scoped({ ref: { ref: "target", slot: slots.get(id)! } as ResolvedRef });
        if (node.path === at) return { evaluated: "selection" };
        const trigger = triggerOf(node.path);
        if (trigger) return { ref: { ref: "event-object", role: trigger } };
        // The object or player chosen by an earlier operand of this block.
        return { ref: { ref: "step-result", step: rel(node.path), role: "selection" } };
      }
      case "derived-object":
        return node.role === "attached-to" && local(node.of) === "/source#object" ? scoped({ ref: { ref: "attached-to" } as ResolvedRef }) : { unresolved: name };
      case "derived-player": {
        if (!RELATIONS.has(node.role) || depth > 4) return { unresolved: name };
        const ofNode = nodes.get(local(node.of));
        // "Their controllers": each member of a set evaluated at another operand.
        const of: Resolved = ofNode?.node === "selection" && ofNode.selection !== "target" && inBlock(ofNode.path) && !triggerOf(ofNode.path)
          ? { ref: { ref: "step-result", step: rel(ofNode.path), role: "selection" } } : resolve(local(node.of), at, depth + 1);
        return "ref" in of && !("scope" in of && of.scope !== undefined)
          ? scoped({ derived: { relation: node.role as PlayerRelation, of: of.ref } }) : { unresolved: `${name}<${"unresolved" in of ? of.unresolved : "compound"}>` };
      }
      case "event-result": {
        const trigger = eventRole(node.event);
        if (trigger && node.result === "objects") return { ref: { ref: "event-object", role: trigger.role } };
        if (trigger && node.result === "player") return { ref: { ref: "event-player", role: trigger.role } };
        const step = trigger ? rel(trigger.path) : owningStep(rel(node.path));
        const role = node.result as StepResultRole;
        return step && ["objects", "player", "quantity"].includes(role) ? { ref: { ref: "step-result", step, role } } : { unresolved: `${name}:${node.result}` };
      }
      case "event-participant":
        return { ref: { ref: "event-object", role: node.role } };
      case "battlefield-entry-proposal":
        return { ref: { ref: "entering-object" } };
      case "announced-number":
        return scope === undefined ? { ref: { ref: "announced-x" } } : { unresolved: `${name}:nested` };
      case "defined-number":
      case "instruction-member":
        return { ref: { ref: "bound-var", id: rel(node.path) } };
      case "recipient":
        return { ref: { ref: "source" }, scope: rel(node.path) };
      case "remainder": {
        const step = owningStep(rel(node.path));
        return step ? { ref: { ref: "step-result", step, role: "remainder" } } : { unresolved: name };
      }
      case "value-choice": {
        const step = owningStep(rel(node.path));
        return step ? { ref: { ref: "step-result", step, role: "choice" } } : { unresolved: name };
      }
      default:
        return LINKED_POOLS.has(node.node) ? { ref: { ref: "linked-pool", id: rel(node.path) } } : { unresolved: name };
    } })();
    // Every scoped operand names the nested ability whose event or step it uses.
    return ("ref" in result || "derived" in result) && result.scope === undefined && scope !== undefined ? { ...result, scope } : result;
  };

  const refs: RefEntry[] = [];
  for (const reference of analysis.references) {
    if (!inBlock(reference.path)) continue;
    const path = rel(reference.path);
    if (reference.binding === "unresolved") {
      refs.push({ path, unresolved: reference.code });
      residuals.add(`ref:${reference.code}`);
      continue;
    }
    const resolved = resolve(local(reference.target), reference.path);
    refs.push({ path, ...resolved } as RefEntry);
    if ("unresolved" in resolved) residuals.add(`ref:${resolved.unresolved}`);
  }
  refs.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  for (const diagnostic of analysis.diagnostics) if (inBlock(diagnostic.path)) residuals.add(`diagnostic:${diagnostic.code}`);

  const printed: { path: string; effect: StepEffect }[] = [];
  collectSteps(ability, "", printed);
  const events = new Set(analysis.nodes.flatMap(node => node.node === "event" && inBlock(node.path) ? [`${rel(node.path)} ${node.operation}`] : []));
  const steps = printed.map(({ path, effect }): StepEntry => {
    if (isStructuralStep(effect)) return { path, astEffect: effect, scope: STRUCTURAL_STEPS[effect] };
    if (events.has(`${path} ${effect}`)) return { path, astEffect: effect, event: effect };
    residuals.add(`step:${effect}`);
    return { path, astEffect: effect, unaccounted: true };
  });
  const spans = line ? blockSpans(line) : { unsupported: "missing block line" };
  if ("unsupported" in spans) residuals.add(`spans:${spans.unsupported}`);
  const engineFeatures = [...new Set(analysis.engineFeatures.filter(need => inBlock(need.path)).map(need => need.feature))].sort();
  return { refs, steps, spans, engineFeatures, residuals: [...residuals].sort() };
}
