// Card-level entry point: source faces, kickers, X and per-ability analysis.
import { isDeepStrictEqual } from "node:util";
import type { ObjectFilter } from "../../ast.ts";
import type { BindingSourceFace, CardBindingAnalysis, CardBindingInput } from "../types.ts";
import { child } from "./context.ts";
import { FACE_LAYOUTS, costHasX, definesX, isRecord } from "./predicates.ts";
import { Analyzer } from "./analyzer.ts";

function validFace(value: unknown): value is BindingSourceFace {
  return isRecord(value) && typeof value.name === "string" && value.name.length > 0 &&
    (value.type_line == null || typeof value.type_line === "string") &&
    (value.mana_cost == null || typeof value.mana_cost === "string");
}
export function analyzeCardBindings(input: CardBindingInput): CardBindingAnalysis {
  const analyzer = new Analyzer(typeof input.identity === "string" ? input.identity : "invalid-source-identity");
  if (typeof input.identity !== "string" || !input.identity || !Array.isArray(input.faces) || !isRecord(input.source) || !validFace(input.source) ||
      typeof input.source.layout !== "string" || !input.source.layout ||
      !(input.source.card_faces == null || (Array.isArray(input.source.card_faces) &&
        input.source.card_faces.length > 0 && input.source.card_faces.every(validFace))) ||
      (FACE_LAYOUTS.has(input.source.layout) && input.source.card_faces?.length !== 2)) {
    analyzer.diagnostic("invalid-source", "/source", "Missing or invalid source identity/layout/face metadata.", []);
    return analyzer.finish();
  }
  const source = analyzer.node({ id: analyzer.id("/source", "object"), path: "/source", node: "source",
    identity: input.identity, rules: ["201.5"] });
  const faces = input.source.card_faces ?? [input.source];
  for (const [index, metadata] of faces.entries()) {
    const path = `/faces/${index}`;
    const matches = input.faces.filter(face => face.sourceFaceIndex === index);
    if (matches.length !== 1) {
      analyzer.diagnostic("missing-face", path, "Exactly one parse result is required for every source face.", []);
      continue;
    }
    const result = matches[0].result;
    const mainTypes = (metadata.type_line ?? "").split(/—|\s-\s/u)[0];
    if (/\b(?:Artifact|Battle|Creature|Enchantment|Land|Planeswalker)\b/.test(mainTypes) && !/\b(?:Instant|Sorcery)\b/.test(mainTypes))
      analyzer.permanentFaces.add(analyzer.id(path, "face"));
    const soleKickerX = result?.ok && analyzer.permanentFaces.has(analyzer.id(path, "face")) &&
      !/\{X\}/.test(metadata.mana_cost ?? "") && !definesX(result.abilities) &&
      !result.abilities.some(ability => ability.kind === "additional-cost" && costHasX(ability.costs)) &&
      result.abilities.flatMap(ability => ability.kind === "keywords" ? ability.keywords : [])
        .filter(keyword => keyword.keyword === "kicker" || keyword.keyword === "multikicker").length === 1;
    if (matches[0].result?.ok) matches[0].result.abilities.forEach((ability, abilityIndex) => {
      if (ability.kind !== "keywords") return;
      ability.keywords.forEach((keyword, keywordIndex) => {
        if (keyword.keyword !== "kicker" && keyword.keyword !== "multikicker") return;
        const list = analyzer.kickers.get(analyzer.id(path, "face")) ?? [];
        const at = `${path}/abilities/${abilityIndex}/keywords/${keywordIndex}`;
        const costs = "additionalCosts" in keyword && Array.isArray(keyword.additionalCosts) ? keyword.additionalCosts.map(cost => JSON.stringify(cost))
          : ["cost" in keyword ? JSON.stringify(keyword.cost) : ""];
        list.push({ id: analyzer.id(at, "keyword"), you: analyzer.id(at, "you"), costs,
          ...(soleKickerX && keyword.keyword === "kicker" && isDeepStrictEqual(keyword.cost, [{ cost: "mana", symbols: ["X"] }])
            ? { announcedX: analyzer.id(at, "announced-x") } : {}) });
        analyzer.kickers.set(analyzer.id(path, "face"), list);
      });
    });
    if (/\{X\}/.test(metadata.mana_cost ?? "") || (matches[0].result?.ok && matches[0].result.abilities.some(ability => ability.kind === "additional-cost" && /"X"|"amount":"x"/.test(JSON.stringify(ability.costs)))))
      analyzer.xFaces.add(analyzer.id(path, "face"));
    const types = mainTypes.toLowerCase().split(/\s+/).filter(word => ["artifact", "battle", "creature", "enchantment", "instant", "land", "planeswalker", "sorcery", "kindred"].includes(word));
    analyzer.faceTypes.set(analyzer.id(path, "face"), { types: types as NonNullable<ObjectFilter["types"]> });
    const face = analyzer.node({ id: analyzer.id(path, "face"), path, node: "face", source,
      sourceFaceIndex: index, name: metadata.name, layout: input.source.layout,
      ...(metadata.type_line != null ? { typeLine: metadata.type_line } : {}),
      ...(metadata.mana_cost != null ? { manaCost: metadata.mana_cost } : {}), rules: ["712"] });
    if (!result?.ok) {
      analyzer.diagnostic("unparsed-face", path, "A missing or failed structural parse cannot have complete reference binding.", []);
      continue;
    }
    if (result.name !== metadata.name) {
      analyzer.diagnostic("face-mismatch", path, "The parsed face name does not match source metadata.", []);
      continue;
    }
    result.abilities.forEach((ability, abilityIndex) => analyzer.abilityAsts.set(child(child(path, "abilities"), abilityIndex), ability));
    result.abilities.forEach((ability, abilityIndex) => analyzer.ability(ability,
      child(child(path, "abilities"), abilityIndex), face, source, faces.length === 1 ? result.abilities : undefined));
  }
  for (const face of input.faces) if (!Number.isInteger(face.sourceFaceIndex) || face.sourceFaceIndex < 0 || face.sourceFaceIndex >= faces.length)
    analyzer.diagnostic("face-mismatch", "/faces", "A supplied parse references a nonexistent source face.", []);
  return analyzer.finish();
}
