// Generate the compiler's typed archive AST from the parser's TypeScript
// source: `ParseCardResult` (ast.ts) and the R4 `CardResolution`
// (semantic/resolution.ts). Rust never mirrors these shapes by hand.
//
//   node scripts/generate-rust-ast.ts          rewrite the committed module
//   node scripts/generate-rust-ast.ts --check  fail if it is stale (engine checks)
//
// Every union decodes by an explicit discriminant computed here (a literal tag
// property, then the presence of a member-only key, then the JSON kind); a
// union no discriminant separates is a generation error, never a serde
// `untagged` trial. Structs reject unknown fields. Only `typescript` (an
// existing development dependency) is used; the output needs serde/serde_json.
import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const SOURCES = ["packages/oracle-parser/src/ast.ts", "packages/oracle-parser/src/semantic/resolution.ts",
  "packages/oracle-parser/src/semantic/spans.ts", "packages/oracle-parser/src/semantic/step-vocabulary.ts"];
const OUTPUT = "packages/game-engine/crates/oracle-ast/src/generated.rs";
const ROOTS: [string, string][] = [["packages/oracle-parser/src/ast.ts", "ParseCardResult"], ["packages/oracle-parser/src/semantic/resolution.ts", "CardResolution"]];

const KEYWORDS = new Set(["as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
  "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true", "type",
  "unsafe", "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro", "override", "priv",
  "typeof", "unsized", "virtual", "yield", "try", "gen"]);
const pascal = (text: string) => {
  const words = text.replace(/[^A-Za-z0-9]+/g, " ").replace(/([a-z0-9])([A-Z])/g, "$1 $2").trim().split(/\s+/).filter(Boolean);
  const name = words.map(w => w[0].toUpperCase() + w.slice(1).toLowerCase()).join("");
  return /^[0-9]/.test(name) ? `N${name}` : name === "Self" ? "SelfObject" : name || "Empty";
};
const snake = (text: string) => {
  const name = text.replace(/([a-z0-9])([A-Z])/g, "$1_$2").replace(/[^A-Za-z0-9]+/g, "_").toLowerCase().replace(/^_|_$/g, "");
  return KEYWORDS.has(name) ? `r#${name}` : /^[0-9]/.test(name) ? `n_${name}` : name;
};

type Literal = string | number | boolean;
interface Member { type: ts.Type; name: string }

export function generate(): string {
  const files = SOURCES.map(file => path.join(ROOT, file));
  const program = ts.createProgram(files, { strict: true, noEmit: true, allowImportingTsExtensions: true, target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext });
  const checker = program.getTypeChecker();
  const names = new Map<ts.Type, string>();
  const used = new Set<string>();
  const defs: string[] = [];
  const literalEnums = new Map<string, string>();
  const helpers = new Set<string>();

  const unique = (base: string) => {
    let name = base, n = 2;
    while (used.has(name)) name = `${base}${n++}`;
    used.add(name);
    return name;
  };
  const isUndefined = (t: ts.Type) => (t.flags & (ts.TypeFlags.Undefined | ts.TypeFlags.Void)) !== 0;
  const isNull = (t: ts.Type) => (t.flags & ts.TypeFlags.Null) !== 0;
  const literalOf = (t: ts.Type): Literal | undefined => {
    if (t.isStringLiteral()) return t.value;
    if (t.isNumberLiteral()) return t.value;
    if (t.flags & ts.TypeFlags.BooleanLiteral) return checker.typeToString(t) === "true";
    return undefined;
  };
  // A union written as `A | B` where A is itself a named union keeps A as one
  // member (TypeScript flattens it but records the written origin).
  const parts = (t: ts.Type): ts.Type[] => {
    const origin = t.isUnion() ? (t as ts.UnionType & { origin?: ts.Type }).origin : undefined;
    const written = origin?.isUnion() ? origin.types : t.isUnion() ? t.types : [t];
    // Intersections that reduce to never (an Exclude of an origin member) are no member.
    return written.filter(x => !isUndefined(x) && !(x.flags & ts.TypeFlags.Never) && checker.typeToString(x) !== "never");
  };
  const isUnionObject = (t: ts.Type) => t.isUnion() && t.types.filter(x => !isUndefined(x)).every(x => isObject(x));
  const isObject = (t: ts.Type): boolean => (t.flags & ts.TypeFlags.Object) !== 0 && !checker.isArrayType(t) && !checker.isTupleType(t) || t.isIntersection();
  const declaredName = (t: ts.Type): string | undefined => {
    if (t.aliasSymbol && !t.aliasTypeArguments?.length) return t.aliasSymbol.name;
    return t.flags & ts.TypeFlags.Object && (t as ts.ObjectType).objectFlags & ts.ObjectFlags.Interface ? t.getSymbol()?.name : undefined;
  };
  // A property typed `?: never` (or `?: undefined`) must be absent: it is not a field.
  const props = (t: ts.Type) => checker.getPropertiesOfType(t).map(symbol => {
    const declaration = symbol.valueDeclaration ?? symbol.declarations?.[0];
    const type = declaration ? checker.getTypeOfSymbolAtLocation(symbol, declaration) : checker.getTypeOfSymbol(symbol);
    return { name: symbol.name, type, optional: (symbol.flags & ts.SymbolFlags.Optional) !== 0 };
  }).filter(p => parts(p.type).some(x => !(x.flags & ts.TypeFlags.Never)));
  const literalSet = (t: ts.Type): Literal[] | undefined => {
    const values = parts(t).map(literalOf);
    return values.every(v => v !== undefined) ? values as Literal[] : undefined;
  };

  /** A unit enum for a set of string literals, shared by identical sets. */
  const stringEnum = (values: string[], hint: string): string => {
    const key = JSON.stringify([...values].sort());
    const known = literalEnums.get(key);
    if (known) return known;
    const name = unique(values.length === 1 ? `Lit${pascal(values[0])}` : hint);
    literalEnums.set(key, name);
    const variants = values.map(v => `    #[serde(rename = ${JSON.stringify(v)})]\n    ${pascal(v)},`);
    const arms = values.map(v => `            ${name}::${pascal(v)} => ${JSON.stringify(v)},`);
    // A single literal is a marker; only a choice needs its spelling back.
    const spelling = values.length > 1 ? `\nimpl ${name} {\n    /// The archived spelling.\n    pub fn as_str(self) -> &'static str {\n        match self {\n${arms.join("\n")}\n        }\n    }\n}` : "";
    defs.push(`#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]\npub enum ${name} {\n${variants.join("\n")}\n}${spelling}`);
    return name;
  };

  const rust = (t: ts.Type, hint: string, boxed = true): string => {
    const members = parts(t);
    if (members.length !== (t.isUnion() ? t.types.length : 1) && members.length === 1) return rust(members[0], hint, boxed);
    if (members.some(isNull)) {
      const rest = members.filter(m => !isNull(m));
      if (!rest.length) throw new Error(`${hint}: null-only type`);
      return `Option<${rest.length === 1 ? rust(rest[0], hint, boxed) : named(t, hint, rest, boxed)}>`;
    }
    if (members.length === 2 && members.every(m => m.flags & ts.TypeFlags.BooleanLiteral)) return "bool";
    if (members.length > 1) return named(t, hint, members, boxed);
    const one = members[0];
    if (one.flags & ts.TypeFlags.String) return "String";
    if (one.flags & ts.TypeFlags.Number) return "i64";
    if (one.flags & ts.TypeFlags.Boolean) return "bool";
    const literal = literalOf(one);
    if (typeof literal === "string") return stringEnum([literal], hint);
    if (typeof literal === "number") { helpers.add("Int"); return `Int<${literal}>`; }
    if (typeof literal === "boolean") { helpers.add(literal ? "True" : "False"); return literal ? "True" : "False"; }
    // A tuple with a rest element ([A, A, ...A[]]) is a list of its element.
    const tupleTarget = checker.isTupleType(one) ? (one as ts.TypeReference).target as ts.TupleType : undefined;
    if (tupleTarget?.hasRestElement) return `Vec<${rust(checker.getTypeArguments(one as ts.TypeReference)[0], `${hint}Item`, false)}>`;
    if (checker.isTupleType(one)) {
      const items = checker.getTypeArguments(one as ts.TypeReference).map((item, index) => rust(item, `${hint}${index}`, false));
      return `(${items.join(", ")}${items.length === 1 ? "," : ""})`;
    }
    if (checker.isArrayType(one)) return `Vec<${rust(checker.getTypeArguments(one as ts.TypeReference)[0], `${hint}Item`, false)}>`;
    if (isObject(one)) {
      const index = checker.getIndexInfosOfType(one);
      if (index.length && !checker.getPropertiesOfType(one).length) return `std::collections::BTreeMap<String, ${rust(index[0].type, `${hint}Value`, false)}>`;
      return named(one, hint, [one], boxed);
    }
    throw new Error(`${hint}: unsupported type ${checker.typeToString(one)}`);
  };

  const named = (t: ts.Type, hint: string, members: ts.Type[], boxed: boolean): string => {
    const existing = names.get(t);
    const wrap = (name: string) => boxed ? `Box<${name}>` : name;
    if (existing) return wrap(existing);
    const flat = t.isUnion() ? t.types.filter(x => !isUndefined(x) && !isNull(x)) : [];
    if (flat.length > 1 && flat.every(m => m.isStringLiteral())) return stringEnum(flat.map(m => (m as ts.StringLiteralType).value), declaredName(t) ?? hint);
    const name = unique(declaredName(t) ?? hint);
    names.set(t, name);
    if (members.length === 1) defs.push(struct(members[0], name, new Set()));
    else defs.push(union(members, name));
    return wrap(name);
  };

  const struct = (t: ts.Type, name: string, omit: Set<string>, optional = new Set<string>()): string => {
    const fields = props(t).filter(p => !omit.has(p.name)).map(p => {
      const type = rust(p.type, `${name}${pascal(p.name)}`);
      const isOptional = p.optional || optional.has(p.name);
      // serde reads a raw identifier unraw and a missing Option as None.
      const field = snake(p.name);
      const rename = field.replace(/^r#/, "") === p.name ? "" : `    #[serde(rename = ${JSON.stringify(p.name)})]\n`;
      return `${rename}    pub ${field}: ${isOptional ? `Option<${type}>` : type},`;
    });
    return `#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]\n#[serde(deny_unknown_fields)]\npub struct ${name} {\n${fields.join("\n")}\n}`;
  };

  /** The JSON shape each member accepts at the top level. */
  const kindOf = (t: ts.Type): "string" | "number" | "boolean" | "array" | "object" => {
    if (t.flags & (ts.TypeFlags.String | ts.TypeFlags.StringLiteral)) return "string";
    if (t.flags & (ts.TypeFlags.Number | ts.TypeFlags.NumberLiteral)) return "number";
    if (t.flags & (ts.TypeFlags.Boolean | ts.TypeFlags.BooleanLiteral)) return "boolean";
    if (checker.isArrayType(t) || checker.isTupleType(t)) return "array";
    if (isObject(t) || isUnionObject(t)) return "object";
    if (t.isUnion() && t.types.every(x => isUndefined(x) || x.isStringLiteral())) return "string";
    throw new Error(`unsupported union member ${checker.typeToString(t)}`);
  };

  type Tree = { leaf: number } | { tag: string; cases: { values: Literal[]; tree: Tree }[] } | { has: string; yes: Tree; no: Tree };
  const union = (written: ts.Type[], name: string): string => {
    // "At least one of": members with the same fields and field types that
    // differ only in which fields are optional ({ power; toughness? } |
    // { power?; toughness }) are one struct whose decoder requires one of them.
    const signature = (m: ts.Type) => isObject(m) ? props(m).map(p => `${p.name}:${checker.typeToString(checker.getNonNullableType(p.type))}`).sort().join(";") : undefined;
    const members: ts.Type[] = [];
    const atLeastOne = new Map<number, string[]>();
    const bySignature = new Map<string, number>();
    for (const m of written) {
      const key = signature(m);
      const rep = key === undefined ? undefined : bySignature.get(key);
      if (rep === undefined) { if (key !== undefined) bySignature.set(key, members.length); members.push(m); continue; }
      const required = (t: ts.Type) => props(t).filter(p => !p.optional).map(p => p.name);
      const keys = new Set([...(atLeastOne.get(rep) ?? required(members[rep])), ...required(m)]);
      atLeastOne.set(rep, [...keys].sort());
    }
    const variants: Member[] = [];
    const lines: string[] = [];
    const byKind = new Map<string, number[]>();
    members.forEach((m, i) => { const k = kindOf(m); byKind.set(k, [...(byKind.get(k) ?? []), i]); });
    const decoders: string[] = [];
    const memberInfo = members.map(m => kindOf(m) === "object" && !m.isUnion() ? props(m) : []);
    // Tags removed from a member struct because the dispatch already proved them.
    const omitted = members.map(() => new Set<string>());
    const declares = (m: ts.Type, key: string): boolean => m.isUnion()
      ? m.types.some(x => !isUndefined(x) && declares(x, key)) : isObject(m) && props(m).some(p => p.name === key);
    // Dispatch works on units: each object member, or each object constituent
    // of a member that is itself a named union (whose own decoder re-dispatches).
    const flatten = (t: ts.Type): ts.Type[] => t.isUnion() ? t.types.filter(x => !isUndefined(x)).flatMap(flatten) : [t];
    const units = members.flatMap((m, i) => kindOf(m) === "object" ? flatten(m).map(type => ({ member: i, type })) : []);
    const unitInfo = units.map(u => props(u.type));
    const decide = (group: number[], tried: Set<string>): Tree => {
      const owners = [...new Set(group.map(u => units[u].member))];
      if (owners.length === 1) return { leaf: owners[0] };
      const all = group.map(u => new Map(unitInfo[u].map(p => [p.name, p])));
      const candidates = [...all[0].keys()].filter(key => !tried.has(key) && all.every(m => m.get(key) && !m.get(key)!.optional && literalSet(m.get(key)!.type)));
      for (const key of candidates) {
        const cases = new Map<string, Set<number>>();
        group.forEach((u, position) => {
          for (const value of literalSet(all[position].get(key)!.type)!) cases.set(JSON.stringify(value), new Set([...(cases.get(JSON.stringify(value)) ?? []), u]));
        });
        if ([...cases.values()].every(g => g.size === group.length)) continue;
        const grouped = new Map<string, { values: Literal[]; units: number[] }>();
        for (const [value, us] of cases) {
          const id = [...us].join(",");
          const entry = grouped.get(id) ?? { values: [], units: [...us] };
          entry.values.push(JSON.parse(value) as Literal);
          grouped.set(id, entry);
        }
        for (const [position, u] of group.entries()) {
          const i = units[u].member;
          const values = literalSet(all[position].get(key)!.type)!;
          const alone = [...grouped.values()].some(g => g.units.length === 1 && g.units[0] === u);
          if (values.length === 1 && alone && !members[i].isUnion()) omitted[i].add(key);
        }
        return { tag: key, cases: [...grouped.values()].map(g => ({ values: g.values, tree: decide(g.units, new Set([...tried, key])) })) };
      }
      // Presence of a key required by some units and not declared by the rest.
      const keys = [...new Set(all.flatMap(m => [...m.keys()]))].sort();
      for (const key of keys) {
        const yes = group.filter((_, p) => all[p].get(key) && !all[p].get(key)!.optional);
        const no = group.filter((_, p) => !all[p].has(key));
        if (yes.length && no.length && yes.length + no.length === group.length) return { has: key, yes: decide(yes, tried), no: decide(no, tried) };
      }
      const describe = (u: number) => `${checker.typeToString(units[u].type).slice(0, 120)} {${unitInfo[u].map(p => `${p.name}${p.optional ? "?" : ""}${literalSet(p.type) ? `=${JSON.stringify(literalSet(p.type))}` : ""}`).join(", ")}}`;
      throw new Error(`${name}: no explicit discriminant separates (tried ${[...tried]})\n  ${group.map(describe).join("\n  ")}`);
    };
    const tree = units.length ? decide(units.map((_, u) => u), new Set()) : undefined;
    members.forEach((m, i) => {
      if (m.isUnion() && (kindOf(m) === "object" || kindOf(m) === "string")) { variants.push({ type: m, name: unique(`${declaredName(m) ?? `Variant${i}`}`).replace(/\d+$/, "") }); used.delete(variants[i].name); return; }
      const literal = literalOf(m);
      const base = typeof literal === "string" ? pascal(literal) : typeof literal === "number" ? (literal < 0 ? `Minus${-literal}` : `N${literal}`) : kindOf(m) === "object" ? (() => {
        const tag = memberInfo[i].find(p => omitted[i].has(p.name));
        const value = tag ? literalSet(tag.type)![0] : undefined;
        if (value !== undefined) return pascal(String(value));
        // Otherwise the key only this member declares ({ ref } | { derived }).
        const own = memberInfo[i].find(p => !p.optional && members.every((other, j) => j === i || !declares(other, p.name)));
        return declaredName(m) ?? (own ? pascal(own.name) : `Variant${i}`);
      })() : pascal(kindOf(m));
      let variant = base, n = 2;
      while (variants.some(v => v.name === variant)) variant = `${base}${n++}`;
      variants.push({ type: m, name: variant });
    });
    for (const [i, m] of members.entries()) {
      const v = variants[i].name;
      if (typeof literalOf(m) === "string" || typeof literalOf(m) === "number") lines.push(`    ${v},`);
      else if (m.isUnion()) lines.push(`    ${v}(${rust(m, `${name}${v}`, true)}),`);
      else if (kindOf(m) === "object") {
        // Not registered by type: this struct omits the tags its dispatch proved.
        const memberName = unique(`${name}${v}`);
        defs.push(struct(m, memberName, omitted[i], new Set(atLeastOne.get(i))));
        lines.push(`    ${v}(Box<${memberName}>),`);
      } else lines.push(`    ${v}(${rust(m, `${name}${v}`, false)}),`);
    }
    const leaf = (i: number) => {
      const v = variants[i].name;
      const omit = [...omitted[i]].map(key => `object.remove(${JSON.stringify(key)});`).join(" ");
      const keys = atLeastOne.get(i);
      const guard = keys ? `if ![${keys.map(k => JSON.stringify(k)).join(", ")}].iter().any(|k| object.contains_key(*k)) { return Err(format!("${name}::${v}: requires one of ${keys.join(", ")}")).map_err(D::Error::custom); } ` : "";
      return `{ ${omit}${guard}strict(Value::Object(object)).map(${name}::${v}).map_err(|e| format!("${name}::${v}: {e}")) }`;
    };
    const tagMatch = (literal: Literal) => `Some(${JSON.stringify(JSON.stringify(literal))})`;
    const render = (t: Tree, depth: number): string => {
      const pad = "    ".repeat(depth);
      if ("leaf" in t) return leaf(t.leaf);
      const block = (code: string) => code.startsWith("{") ? code : `{ ${code} }`;
      if ("has" in t) return `if object.contains_key(${JSON.stringify(t.has)}) ${block(render(t.yes, depth))} else ${block(render(t.no, depth))}`;
      const arms = t.cases.map(c => `${pad}    ${c.values.map(tagMatch).join(" | ")} => ${render(c.tree, depth + 1)},`);
      return `match tag(&object, ${JSON.stringify(t.tag)}).as_deref() {\n${arms.join("\n")}\n${pad}    other => Err(format!("${name}: unknown ${t.tag} {other:?}")),\n${pad}}`;
    };
    const stringLits = members.flatMap((m, i) => typeof literalOf(m) === "string" ? [[literalOf(m) as string, variants[i].name]] : []);
    // A named set of string literals stays one member holding its own enum.
    const literalUnions = members.flatMap((m, i) => m.isUnion() && kindOf(m) === "string"
      ? [[(m.types.filter(x => x.isStringLiteral()) as ts.StringLiteralType[]).map(x => x.value), variants[i].name] as const] : []);
    const stringAny = members.findIndex(m => (m.flags & ts.TypeFlags.String) !== 0);
    const arms: string[] = [];
    if (stringLits.length || stringAny >= 0 || literalUnions.length) {
      const cases = [...stringLits.map(([lit, v]) => `            ${JSON.stringify(lit)} => Ok(${name}::${v}),`),
        ...literalUnions.map(([lits, v]) => `            ${lits.map(l => JSON.stringify(l)).join(" | ")} => strict(Value::String(s)).map(${name}::${v}).map_err(|e| format!("${name}: {e}")),`)];
      const fallback = stringAny >= 0 ? `            _ => Ok(${name}::${variants[stringAny].name}(s)),` : `            other => Err(format!("${name}: unknown string {other:?}")),`;
      arms.push(`        Value::String(s) => match s.as_str() {\n${cases.join("\n")}\n${fallback}\n        },`);
    }
    // Tuples of different printed lengths are told apart by their length.
    const tuples = members.flatMap((m, i) => checker.isTupleType(m) ? [i] : []);
    const lengths = tuples.map(i => checker.getTypeArguments(members[i] as ts.TypeReference).length);
    if (tuples.length > 1 && new Set(lengths).size === tuples.length && tuples.length === members.filter(m => kindOf(m) === "array").length) {
      for (const [n, i] of tuples.entries())
        arms.push(`        Value::Array(items) if items.len() == ${lengths[n]} => strict(Value::Array(items)).map(${name}::${variants[i].name}).map_err(|e| format!("${name}: {e}")),`);
    }
    const numberLits = members.flatMap((m, i) => typeof literalOf(m) === "number" ? [i] : []);
    for (const i of numberLits) arms.push(`        Value::Number(n) if n.as_i64() == Some(${literalOf(members[i])}) => Ok(${name}::${variants[i].name}),`);
    for (const kind of ["number", "boolean", "array"] as const) {
      const index = members.findIndex((m, i) => kindOf(m) === kind && !numberLits.includes(i));
      if (index < 0 || (kind === "array" && tuples.length > 1)) continue;
      if (members.filter((m, i) => kindOf(m) === kind && !numberLits.includes(i)).length > 1) throw new Error(`${name}: several ${kind} members`);
      const pattern = kind === "number" ? "Value::Number(_)" : kind === "boolean" ? "Value::Bool(_)" : "Value::Array(_)";
      arms.push(`        value @ ${pattern} => strict(value).map(${name}::${variants[index].name}).map_err(|e| format!("${name}: {e}")),`);
    }
    if (tree) arms.push(`        Value::Object(mut object) => ${render(tree, 2)},`);
    arms.push(`        other => Err(format!("${name}: unexpected {other}")),`);
    decoders.push(`impl<'de> Deserialize<'de> for ${name} {\n    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {\n        #[allow(unused_mut)]\n        match Value::deserialize(deserializer)? {\n${arms.join("\n")}\n        }\n        .map_err(D::Error::custom)\n    }\n}`);
    return `#[derive(Debug, Clone, PartialEq, Eq)]\npub enum ${name} {\n${lines.join("\n")}\n}\n${decoders.join("\n")}`;
  };

  for (const [file, root] of ROOTS) {
    const source = program.getSourceFile(path.join(ROOT, file));
    if (!source) throw new Error(`missing ${file}`);
    const symbol = checker.getSymbolAtLocation(source)?.exports?.get(root as ts.__String);
    if (!symbol) throw new Error(`missing export ${root}`);
    rust(checker.getDeclaredTypeOfSymbol(symbol), root, false);
  }
  const hash = createHash("sha256");
  for (const file of SOURCES) hash.update(file).update("\0").update(fs.readFileSync(path.join(ROOT, file)));
  hash.update(fs.readFileSync(fileURLToPath(import.meta.url)));
  const header = `// @generated by scripts/generate-rust-ast.ts; do not edit.\n// Sources: ${SOURCES.join(", ")}\n// Source SHA-256: ${hash.digest("hex")}\n` +
    `// Check: node scripts/generate-rust-ast.ts --check\n#![allow(clippy::all, dead_code, non_camel_case_types)]\nuse super::{strict, tag, Deserialize, Deserializer, Value${[...helpers].sort().map(h => `, ${h}`).join("")}};\nuse serde::de::Error as _;\n`;
  return `${header}\n${defs.join("\n\n")}\n`;
}

function main(): void {
  const check = process.argv.includes("--check");
  const output = path.join(ROOT, OUTPUT);
  const text = generate();
  if (check) {
    if (!fs.existsSync(output) || fs.readFileSync(output, "utf8") !== text) {
      console.error(`${OUTPUT} is stale; run: node scripts/generate-rust-ast.ts`);
      process.exitCode = 1;
    }
    return;
  }
  fs.writeFileSync(output, text);
  console.log(`wrote ${OUTPUT} (${text.split("\n").length} lines)`);
}

main();
