// Printed-symbol and narrow direct-source analysis, not a mana/rules simulator.
// Symbols: https://scryfall.com/docs/api/card-symbols (CR 107.4).
// Intrinsic Land subtypes: CR 305.6; relative output needs context (CR 106.7):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
// Faces: https://scryfall.com/docs/api/cards and /docs/api/layouts. Never sum
// alternative faces as simultaneous resources or substitute colour identity.
export const MANA_TYPES = ["W", "U", "B", "R", "G", "C"] as const;
export type ManaType = typeof MANA_TYPES[number];
const types = () => ({ W: 0, U: 0, B: 0, R: 0, G: 0, C: 0 });
const costs = () => ({ fixed_symbols: types(), generic_units: 0, flexible_symbols: {} as Record<string, number>, variable_symbols: { X: 0, Y: 0, Z: 0 }, snow_symbols: 0, no_printed_cost_nonland_copies: 0 });
const sources = () => ({ by_type: types(), any_color_copies: 0 });
const bucket = () => ({ costs: costs(), sources: { land: sources(), nonland: sources() } });
interface Face { mana_cost: string | null; type_line: string; oracle_text: string }
export interface ManaCard extends Face { oracle_id: string; name: string; quantity: number; role: string; maybe: number; layout: string; faces_json: string | null }
interface Issue { oracle_id: string; name: string; quantity: number; face_index: number; domain: "cost" | "source"; reason: string; excerpt: string }
interface Evidence { oracle_id: string; name: string; quantity: number; face_index: number; kind: "land" | "nonland"; types: ManaType[]; any_color: boolean; evidence: string[] }
const MULTIFACE = new Set(["split", "flip", "transform", "modal_dfc", "adventure", "aftermath", "meld", "double_faced_token", "reversible_card"]);
const SUPPORTED = new Set(["normal", "split", "flip", "transform", "modal_dfc", "adventure", "aftermath", "leveler", "saga", "class", "case", "mutate", "prototype", "battle"]);
const BASIC: Record<string, ManaType> = { Plains: "W", Island: "U", Swamp: "B", Mountain: "R", Forest: "G" };
const isLand = (face: Face) => /\bLand\b/.test(face.type_line.split("—")[0]);
const permanent = (face: Face) => /\b(?:Land|Artifact|Creature|Enchantment|Planeswalker|Battle)\b/.test(face.type_line.split("—")[0]);
// A mention of mana value/cost or an unrelated token is not production.
// This only identifies text needing review; it never assigns any output.
const productionText = (text: string) => /\badds?\b[^.\n]*(?:\bmana\b|\{[WUBRGC]\})|\bproduc(?:e[sd]?|ing)\b[^.\n]*\bmana\b|\bmana\b[^.\n]*\bproduc(?:e[sd]?|ing)\b/i.test(text);

/** Remove reminder and quoted/granted ability text without accidentally
 * joining lines. Unbalanced text is explicitly unresolved. */
function unquoted(text: string) {
  let clean = "", depth = 0, quote = "", valid = true;
  for (const char of text) {
    if (depth) { if (char === "(") depth++; if (char === ")") depth--; clean += char === "\n" ? "\n" : " "; continue; }
    if (quote) { if (char === quote) quote = ""; clean += char === "\n" ? "\n" : " "; continue; }
    if (char === "(") { depth++; clean += " "; }
    else if (char === ")") { valid = false; clean += " "; }
    else if (char === '"' || char === "“") { quote = char === "“" ? "”" : '"'; clean += " "; }
    else if (char === "”") { valid = false; clean += " "; }
    else clean += char;
  }
  return { clean, valid: valid && depth === 0 && quote === "" };
}

export function analyzeMana(cards: readonly ManaCard[]) {
  const first_face = bucket(), other_faces = bucket(), issues: Issue[] = [], recognized: Evidence[] = [];
  const coverage = { included_copies: 0, other_face_copies: 0, recognized_source_copies: 0, partial_source_copies: 0, unresolved_source_copies: 0, no_supported_source_pattern_copies: 0, cost_issue_copies: 0, source_reason_copies: {} as Record<string, number> };
  for (const card of cards) {
    if (card.maybe || card.role === "companion") continue;
    const quantity = card.quantity;
    coverage.included_copies += quantity;
    let sourceIssue = false, costIssue = false, hasSource = false;
    const reasons = new Set<string>(), counted = new Set<string>();
    function issue(domain: Issue["domain"], reason: string, face_index: number, excerpt = "") {
      issues.push({ oracle_id: card.oracle_id, name: card.name, quantity, face_index, domain, reason, excerpt: excerpt.slice(0, 200) });
      if (domain === "source") { sourceIssue = true; reasons.add(reason); } else costIssue = true;
    }
    let faces: Face[] = [];
    if (!SUPPORTED.has(card.layout)) {
      issue("cost", "unsupported_layout", 0, card.layout); issue("source", "unsupported_layout", 0, card.layout);
    } else if (card.faces_json !== null) {
      try {
        const parsed: unknown = JSON.parse(card.faces_json);
        if (!Array.isArray(parsed) || !parsed.length || (MULTIFACE.has(card.layout) && parsed.length < 2) || !parsed.every(face => face && typeof face.type_line === "string" && (face.oracle_text === null || typeof face.oracle_text === "string") && (face.mana_cost === null || typeof face.mana_cost === "string"))) throw new Error("incomplete faces");
        faces = parsed.map(face => ({ ...face, oracle_text: face.oracle_text ?? "" }));
      } catch { issue("cost", "malformed_faces", 0); issue("source", "malformed_faces", 0); }
    } else if (MULTIFACE.has(card.layout)) {
      issue("cost", "missing_faces", 0); issue("source", "missing_faces", 0);
    } else faces = [card];

    if (faces.length > 1) coverage.other_face_copies += quantity;
    for (const [index, face] of faces.entries()) {
      const group = index === 0 ? first_face : other_faces;
      if (!isLand(face)) {
        if (!face.mana_cost) group.costs.no_printed_cost_nonland_copies += quantity;
        else {
          const rest = face.mana_cost.replace(/\{([^{}]+)\}/g, (whole, token: string) => {
            if (MANA_TYPES.includes(token as ManaType)) group.costs.fixed_symbols[token as ManaType] += quantity;
            else if (/^\d+$/.test(token) && Number.isSafeInteger(Number(token))) group.costs.generic_units += Number(token) * quantity;
            else if (/^[XYZ]$/.test(token)) group.costs.variable_symbols[token as "X" | "Y" | "Z"] += quantity;
            else if (token === "S") group.costs.snow_symbols += quantity;
            else if (/^(?:[WUBRG]\/[WUBRG]|2\/[WUBRG]|C\/[WUBRG]|[WUBRG]\/P|[WUBRG]\/[WUBRG]\/P)$/.test(token)) group.costs.flexible_symbols[whole] = (group.costs.flexible_symbols[whole] ?? 0) + quantity;
            else issue("cost", "unsupported_symbol", index, whole);
            return "";
          });
          if (rest.trim()) issue("cost", "malformed_cost", index, rest);
        }
      }
      const kind = isLand(face) ? "land" : "nonland", output = new Set<ManaType>(), evidence: string[] = [];
      let any = false;
      if (kind === "land") {
        for (const subtype of (face.type_line.split("—")[1] ?? "").trim().split(/\s+/)) if (BASIC[subtype]) { output.add(BASIC[subtype]); evidence.push(`Land subtype: ${subtype}`); }
      }
      const text = unquoted(face.oracle_text);
      if (!text.valid) issue("source", "unbalanced_text", index, face.oracle_text);
      else for (const raw of text.clean.split("\n")) {
        const line = raw.trim();
        if (!line) continue;
        const direct = permanent(face) && line.match(/^\{T\}: Add (.+)\.$/);
        const value = direct ? direct[1] : "";
        if (value === "one mana of any color") { any = true; evidence.push(line); }
        else if (/^(?:\{[WUBRGC]\})+$/.test(value) || /^\{[WUBRGC]\}(?:(?:,\s*|,?\s+or\s+)\{[WUBRGC]\})+$/.test(value)) {
          for (const symbol of value.matchAll(/\{([WUBRGC])\}/g)) output.add(symbol[1] as ManaType);
          evidence.push(line);
        } else if (productionText(line)) issue("source", "unsupported_mana_text", index, line);
        else if (/\bsearch\b.*\blibrary\b.*\bland\b/i.test(line)) issue("source", "indirect_text", index, line);
      }
      const quotedProduction = [...face.oracle_text.matchAll(/"([^"]*)"|“([^”]*)”/g)].some(match => productionText(match[1] ?? match[2]));
      if (quotedProduction) issue("source", "quoted_or_granted_text", index, face.oracle_text);
      if (kind === "land" && !output.size && !any && !sourceIssue) issue("source", "unassigned_land", index, face.oracle_text);
      if (output.size || any) {
        hasSource = true;
        recognized.push({ oracle_id: card.oracle_id, name: card.name, quantity, face_index: index, kind, types: MANA_TYPES.filter(type => output.has(type)), any_color: any, evidence });
        for (const type of output) {
          const key = `${index === 0 ? "first" : "other"}/${kind}/${type}`;
          if (!counted.has(key)) { group.sources[kind].by_type[type] += quantity; counted.add(key); }
        }
        const key = `${index === 0 ? "first" : "other"}/${kind}/any`;
        if (any && !counted.has(key)) { group.sources[kind].any_color_copies += quantity; counted.add(key); }
      }
    }
    if (hasSource) { coverage.recognized_source_copies += quantity; if (sourceIssue) coverage.partial_source_copies += quantity; }
    else if (sourceIssue) coverage.unresolved_source_copies += quantity;
    else coverage.no_supported_source_pattern_copies += quantity;
    if (costIssue) coverage.cost_issue_copies += quantity;
    for (const reason of reasons) coverage.source_reason_copies[reason] = (coverage.source_reason_copies[reason] ?? 0) + quantity;
  }
  issues.sort((a, b) => a.name.localeCompare(b.name) || a.oracle_id.localeCompare(b.oracle_id) || a.face_index - b.face_index || a.domain.localeCompare(b.domain) || a.reason.localeCompare(b.reason) || a.excerpt.localeCompare(b.excerpt));
  recognized.sort((a, b) => a.name.localeCompare(b.name) || a.oracle_id.localeCompare(b.oracle_id) || a.face_index - b.face_index);
  return { version: 1 as const, scope: "main_with_commanders" as const, first_face, other_faces, coverage, issues, recognized };
}
export type ManaAnalysis = ReturnType<typeof analyzeMana>;
