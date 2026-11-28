// Generate the compiler's archive AST and printed evidence from TypeScript.
// The legacy AST is rooted in `ParseCardResult` (ast.ts) and `CardResolution`
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


const PRINTED_SOURCE = "packages/oracle-parser/src/printed/schema.ts";
const VOCAB_SOURCE = "packages/oracle-parser/src/printed/vocabulary.ts";
const LEXER_SOURCE = "packages/oracle-parser/src/lexer.ts";
const sha256 = (bytes: string | Buffer) => createHash("sha256").update(bytes).digest("hex");
const sourceFile = (file: string) =>
  ts.createSourceFile(file, fs.readFileSync(path.join(ROOT, file), "utf8"), ts.ScriptTarget.ES2022, true);
const lexicalOrder = (a: string, b: string) => a < b ? -1 : a > b ? 1 : 0;

type PrintedShape =
  | { kind: "u32" | "string" | "boolean" | "never" }
  | { kind: "literal"; value: number | boolean }
  | { kind: "strings"; values: string[] }
  | { kind: "ref"; name: string }
  | { kind: "array"; item: PrintedShape }
  | { kind: "tuple"; arity: number; range: [start: number, end: number] | null }
  | { kind: "object"; fields: PrintedField[] }
  | { kind: "tagged"; key: string; cases: { tag: string; fields: PrintedField[] }[] };

interface PrintedField {
  name: string;
  optional: boolean;
  nullable: boolean;
  shape: PrintedShape;
}

// Read the closed subset used by printed/schema.ts. Unsupported forms fail
// generation, including an object union with no disjoint literal discriminator.
export function printedSchema(source = sourceFile(PRINTED_SOURCE)) {
  const unwrap = (node: ts.TypeNode): ts.TypeNode => ts.isParenthesizedTypeNode(node) ? unwrap(node.type) : node;
  const fail = (node: ts.Node, why: string): never => {
    const { line } = source.getLineAndCharacterOfPosition(node.getStart(source));
    throw new Error(`${source.fileName}:${line + 1}: ${why}`);
  };
  const members = (node: ts.TypeNode): ts.TypeNode[] => {
    const type = unwrap(node);
    return ts.isUnionTypeNode(type) ? type.types.flatMap(members) : [type];
  };
  const fields = (nodes: ts.NodeArray<ts.TypeElement>): PrintedField[] =>
    nodes.map(node => {
      if (!ts.isPropertySignature(node) || !node.type ||
          !(ts.isIdentifier(node.name) || ts.isStringLiteral(node.name)))
        return fail(node, "expected a typed property");
      const types = members(node.type);
      const values = types.filter(type => !(ts.isLiteralTypeNode(type) && type.literal.kind === ts.SyntaxKind.NullKeyword));
      const optional = !!node.questionToken;
      const nullable = types.length !== values.length;
      const shape = readTypes(values, node.type);
      if (shape.kind === "never" && (!optional || nullable))
        return fail(node, "never is supported only as an optional forbidden property");
      return { name: node.name.text, optional, nullable, shape };
    });

  // Distribute intersections of object fields across a union's branches.
  // This handles the common wire fields on Edit, PrintedToken, PrintedFact, etc.
  const branches = (node: ts.TypeNode): PrintedField[][] => {
    const type = unwrap(node);
    if (ts.isTypeLiteralNode(type)) return [fields(type.members)];
    if (ts.isUnionTypeNode(type)) return type.types.flatMap(branches);
    if (ts.isIntersectionTypeNode(type))
      return type.types.reduce<PrintedField[][]>(
        (left, next) => left.flatMap(a => branches(next).map(b => [...a, ...b])), [[]]);
    return fail(type, "expected a closed object branch");
  };

  const objectShape = (cases: PrintedField[][], node: ts.Node): PrintedShape => {
    for (const fields of cases) {
      fields.sort((a, b) => lexicalOrder(a.name, b.name));
      if (new Set(fields.map(field => field.name)).size !== fields.length)
        fail(node, "intersection redeclares a property");
    }
    if (cases.length === 1) return { kind: "object", fields: cases[0] };
    for (const { name } of cases[0]) {
      const tagged = cases.flatMap(fields => {
        const field = fields.find(field => field.name === name);
        if (field?.shape.kind !== "strings" || field.optional || field.nullable) return [];
        return [field.shape.values.map(tag => ({ tag, fields: fields.filter(field => field.name !== name) }))];
      });
      if (tagged.length !== cases.length) continue;
      const tags = tagged.flat().sort((a, b) => lexicalOrder(a.tag, b.tag));
      if (new Set(tags.map(({ tag }) => tag)).size === tags.length)
        return { kind: "tagged", key: name, cases: tags };
    }
    return fail(node, "no disjoint required string discriminator");
  };

  function readTypes(types: ts.TypeNode[], at: ts.TypeNode): PrintedShape {
    if (!types.length) return fail(at, "a nullable field must have a non-null type");
    const strings = types.flatMap(type =>
      ts.isLiteralTypeNode(type) && ts.isStringLiteral(type.literal) ? [type.literal.text] : []);
    if (strings.length === types.length)
      return { kind: "strings", values: [...new Set(strings)].sort(lexicalOrder) };
    if (types.length !== 1) return objectShape(types.flatMap(branches), at);

    const [type] = types;
    switch (type.kind) {
      case ts.SyntaxKind.NumberKeyword: return { kind: "u32" };
      case ts.SyntaxKind.StringKeyword: return { kind: "string" };
      case ts.SyntaxKind.BooleanKeyword: return { kind: "boolean" };
      case ts.SyntaxKind.NeverKeyword: return { kind: "never" };
    }
    if (ts.isArrayTypeNode(type)) return { kind: "array", item: readTypes(members(type.elementType), type.elementType) };
    if (ts.isTypeReferenceNode(type) && ts.isIdentifier(type.typeName) && !type.typeArguments)
      return { kind: "ref", name: type.typeName.text };
    if (ts.isLiteralTypeNode(type)) {
      const { literal } = type;
      if (literal.kind === ts.SyntaxKind.TrueKeyword || literal.kind === ts.SyntaxKind.FalseKeyword)
        return { kind: "literal", value: literal.kind === ts.SyntaxKind.TrueKeyword };
      if (ts.isNumericLiteral(literal)) {
        const value = Number(literal.text);
        if (Number.isInteger(value) && value >= 0 && value <= 0xffffffff) return { kind: "literal", value };
      }
    }
    if (ts.isTupleTypeNode(type)) {
      const labels = type.elements.map(element => {
        if (ts.isNamedTupleMember(element) && !element.dotDotDotToken && !element.questionToken &&
            element.type.kind === ts.SyntaxKind.NumberKeyword) return element.name.text;
        return fail(element, "expected a required named u32 tuple item");
      });
      const start = labels.indexOf("start"), end = labels.indexOf("end");
      if (start >= 0 ? end <= start : end >= 0) return fail(type, "range tuple requires start before end");
      return { kind: "tuple", arity: labels.length, range: start >= 0 ? [start, end] : null };
    }
    if (ts.isTypeLiteralNode(type) || ts.isIntersectionTypeNode(type))
      return objectShape(branches(type), at);
    return fail(at, "unsupported printed wire type");
  }

  const entries = source.statements.map(node => {
    if (ts.isInterfaceDeclaration(node) && !node.heritageClauses && !node.typeParameters)
      return [node.name.text, objectShape([fields(node.members)], node)] as const;
    if (ts.isTypeAliasDeclaration(node) && !node.typeParameters)
      return [node.name.text, readTypes(members(node.type), node.type)] as const;
    return fail(node, "expected an interface or type alias");
  });
  return new Map(entries.sort(([a], [b]) => lexicalOrder(a, b)));
}

function printedVocabulary(): [string, string[]][] {
  const tables = (source: ts.SourceFile, select: (node: ts.VariableDeclaration) => boolean) =>
    source.statements.flatMap(node => ts.isVariableStatement(node)
      ? node.declarationList.declarations.filter(select).map((declaration): [string, string[]] => {
        const name = declaration.name.getText(source);
        const init = declaration.initializer;
        const array = init && ts.isAsExpression(init) ? init.expression
          : init && ts.isNewExpression(init) && init.expression.getText(source) === "Set" && init.arguments?.length === 1
            ? init.arguments[0] : init;
        if (!array || !ts.isArrayLiteralExpression(array)) throw new Error(`${name}: expected a vocabulary array`);
        const values = array.elements.map(element => {
          if (!ts.isStringLiteral(element)) throw new Error(`${name}: expected a literal vocabulary item`);
          return element.text;
        });
        if (new Set(values).size !== values.length) throw new Error(`${name}: duplicate vocabulary item`);
        return [name, values.sort(lexicalOrder)];
      }) : []);
  const lexerNames = new Set(["PUNCT_CHARS", "CONTRACTIONS"]);
  const lexer = tables(sourceFile(LEXER_SOURCE), node => lexerNames.has(node.name.getText()));
  if (lexer.length !== lexerNames.size) throw new Error("missing lexer vocabulary table");
  return [...tables(sourceFile(VOCAB_SOURCE), () => true), ...lexer].sort(([a], [b]) => lexicalOrder(a, b));
}

function printedContract() {
  const schema = printedSchema();
  const vocabulary = printedVocabulary();
  const descriptor = JSON.stringify([...schema]);
  const card = schema.get("PrintedCard");
  const version = card?.kind === "object" && card.fields.find(field => field.name === "schemaVersion")?.shape;
  if (!version || version.kind !== "literal" || typeof version.value !== "number")
    throw new Error("PrintedCard requires an integer schemaVersion literal");
  const identity = {
    schemaVersion: version.value,
    schemaSha256: sha256(descriptor),
    vocabularySha256: sha256(JSON.stringify(vocabulary)),
    generatorSha256: sha256(fs.readFileSync(fileURLToPath(import.meta.url))),
  };
  return { schema, vocabulary, descriptor, identity };
}

/** The schema-5 header pins for the printed contract. The writer adds recordsSha256. */
export function printedIdentity() {
  return printedContract().identity;
}

type PrintedContract = ReturnType<typeof printedContract>;
const PRINTED_HEADER = "// @generated by scripts/generate-rust-ast.ts; do not edit.\n";
const rustString = (value: string) => JSON.stringify(value).replace(
  /\\(?:\\|u([0-9a-f]{4})|[bf])/g, (escape, hex: string | undefined) =>
    hex ? `\\u{${hex}}` : escape === "\\b" ? "\\u{8}" : escape === "\\f" ? "\\u{c}" : escape);

export function renderPrinted({ schema, descriptor, identity }: Pick<PrintedContract, "schema" | "descriptor" | "identity">): string {
  const defs = new Map<string, string>();
  const header = [
    PRINTED_HEADER.trimEnd(),
    `// Schema: ${PRINTED_SOURCE}`,
    "use super::printed_decode::*;",
    "pub use super::printed_decode::{Fault, LiteralBool, LiteralU32, Nullable, Presence, Reject};",
    "use serde::Serialize;",
    "use serde_json::Value;",
    "",
    `pub const PRINTED_SCHEMA_VERSION: u32 = ${identity.schemaVersion};`,
    `pub const PRINTED_SCHEMA_SHA256: &str = ${rustString(identity.schemaSha256)};`,
    `pub const PRINTED_GENERATOR_SHA256: &str = ${rustString(identity.generatorSha256)};`,
    `pub const PRINTED_SCHEMA_DESCRIPTOR: &str = ${rustString(descriptor)};`,
  ];

  const derive = "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]";
  const define = (name: string, body: string) => {
    if (defs.has(name)) throw new Error(`duplicate printed Rust type ${name}`);
    defs.set(name, body);
    return name;
  };
  const wire = (name: string, lines: string[]) =>
    `impl Wire for ${name} {\n    fn present(value: &Value, at: &Pointer<'_>) -> Result<Self, Reject> {\n${lines.join("\n")}\n    }\n}`;

  const struct = (name: string, fields: PrintedField[], tag?: string) => {
    const stored = fields.filter(field => field.shape.kind !== "never");
    const declarations: string[] = [];
    const reads: string[] = [];
    for (const { name: key, optional, nullable, shape } of stored) {
      const field = snake(key);
      const type = rustType(shape, name + pascal(key));
      const [wrapper, read, skip] = optional
        ? nullable ? ["Presence", "presence", "Presence::is_omitted"] : ["Option", "optional", "Option::is_none"]
        : nullable ? ["Nullable", "nullable", ""] : ["", "required", ""];
      if (field.replace(/^r#/, "") !== key) declarations.push(`    #[serde(rename = ${rustString(key)})]`);
      if (skip) declarations.push(`    #[serde(skip_serializing_if = ${rustString(skip)})]`);
      declarations.push(`    pub ${field}: ${wrapper ? `${wrapper}<${type}>` : type},`);
      reads.push(`            ${field}: ${read}(fields, ${rustString(key)}, at)?,`);
    }
    const keys = [...(tag === undefined ? [] : [tag]), ...stored.map(field => field.name)];
    const decodeFields = `impl ${name} {\n    fn read_fields(fields: &Object, at: &Pointer<'_>) -> Result<Self, Reject> {\n        known_fields(fields, &[${keys.map(rustString).join(", ")}], at)?;\n        Ok(Self {\n${reads.join("\n")}\n        })\n    }\n}`;
    const decodeValue = tag === undefined ? "\n" + wire(name, ["        Self::read_fields(object(value, at)?, at)"]) : "";
    return define(name, `${derive}\npub struct ${name} {\n${declarations.join("\n")}\n}\n${decodeFields}${decodeValue}`);
  };

  function rustType(shape: PrintedShape, name: string): string {
    switch (shape.kind) {
      case "string": return "String";
      case "u32": return "u32";
      case "boolean": return "bool";
      case "never": throw new Error(`${name}: cannot generate a standalone never type`);
      case "ref":
        if (!schema.has(shape.name)) throw new Error(`${name}: unknown printed type ${shape.name}`);
        return shape.name;
      case "array": return `Vec<${rustType(shape.item, name + "Item")}>`;
      case "literal": return `${typeof shape.value === "number" ? "LiteralU32" : "LiteralBool"}<${shape.value}>`;
      case "object": return struct(name, shape.fields);
      case "tuple": {
        const checks = ["        let items = tuple(value, at)?;"];
        if (shape.range) {
          const [start, end] = shape.range;
          checks.push(`        if items[${start}] > items[${end}] {`,
            `            return Err(Pointer::Index(at, ${end}).reject(Fault::Malformed, "range end precedes start"));`, "        }");
        }
        checks.push("        Ok(Self(items))");
        return define(name, `${derive}\n#[serde(transparent)]\npub struct ${name}(pub [u32; ${shape.arity}]);\n${wire(name, checks)}`);
      }
      case "strings": {
        const used = new Set<string>();
        const variants = shape.values.map(value => {
          const base = pascal(value);
          let variant = base, suffix = 2;
          while (used.has(variant)) variant = base + suffix++;
          used.add(variant);
          return { value: rustString(value), variant };
        });
        const declarations = variants.map(({ value, variant }) => `    #[serde(rename = ${value})]\n    ${variant},`);
        const spelling = variants.map(({ value, variant }) => `            Self::${variant} => ${value},`);
        const reads = variants.map(({ value, variant }) => `            ${value} => Ok(Self::${variant}),`);
        const decodeEnum = wire(name, ["        match text(value, at)? {", ...reads,
          '            _ => Err(at.reject(Fault::Unknown, "unknown enum value")),', "        }"]);
        return define(name, `${derive}\npub enum ${name} {\n${declarations.join("\n")}\n}\nimpl ${name} {\n    pub fn as_str(&self) -> &'static str {\n        match self {\n${spelling.join("\n")}\n        }\n    }\n}\n${decodeEnum}`);
      }
      case "tagged": {
        const variants = shape.cases.map(({ tag, fields }) => {
          const variant = pascal(tag);
          return { tag: rustString(tag), variant, member: struct(name + variant, fields, shape.key) };
        });
        const key = rustString(shape.key);
        const declarations = variants.map(({ tag, variant, member }) => `    #[serde(rename = ${tag})]\n    ${variant}(${member}),`);
        const dispatch = variants.map(({ tag, member, variant }) => `            ${tag} => ${member}::read_fields(fields, at).map(Self::${variant}),`);
        const decodeUnion = wire(name, ["        let fields = object(value, at)?;", `        match tag(fields, ${key}, at)? {`,
          ...dispatch, `            _ => Err(Pointer::Key(at, ${key}).reject(Fault::Unknown, "unknown discriminator")),`, "        }"]);
        return define(name, `${derive}\n#[serde(tag = ${key})]\npub enum ${name} {\n${declarations.join("\n")}\n}\n${decodeUnion}`);
      }
    }
  }

  for (const [name, shape] of schema) {
    const type = rustType(shape, name);
    if (type !== name) define(name, `pub type ${name} = ${type};`);
  }
  for (const root of ["PrintedCard", "PrintedFace"].filter(name => schema.has(name))) {
    defs.set(root, defs.get(root) + `\nimpl ${root} {\n    /// Decode original JSON so duplicate object keys remain observable.\n    pub fn decode(json: &str) -> Result<Self, Reject> {\n        decode(json)\n    }\n}`);
  }
  return [header.join("\n"), ...defs.values(), ""].join("\n\n").trimEnd() + "\n";
}

function renderVocabulary({ identity, vocabulary }: PrintedContract): string {
  const constants = vocabulary.map(([name, words]) =>
    `pub const ${name}: &[&str] = &[${words.map(rustString).join(", ")}];`);
  const table = vocabulary.map(([name]) => `    (${rustString(name)}, ${name}),`);
  return PRINTED_HEADER + `// Sources: ${VOCAB_SOURCE}, ${LEXER_SOURCE}\n\n` +
    `pub const PRINTED_VOCABULARY_SHA256: &str = ${rustString(identity.vocabularySha256)};\n\n` +
    constants.join("\n") + `\n\npub const VOCABULARY_TABLES: &[(&str, &[&str])] = &[\n${table.join("\n")}\n];\n`;
}


/** The legacy decoder freeze, without its source hash line. It lives outside
 * this file so a grammar batch's new pin leaves PRINTED_GENERATOR_SHA256 fixed. */
const LEGACY_DECODER_PIN = "scripts/generate-rust-ast.legacy-decoder.sha256";

function main(): void {
  const check = process.argv.includes("--check");
  const legacy = generate();
  // Freeze the legacy decoder through the R6 bridge. Its source fingerprint
  // covers this generator, so that one line is allowed to change. Each
  // grammar batch updates the pin file (rework plan, format cut 3).
  const legacyHash = /^\/\/ Source SHA-256: [0-9a-f]{64}\n/m;
  const pin = fs.readFileSync(path.join(ROOT, LEGACY_DECODER_PIN), "utf8");
  if (!/^[0-9a-f]{64}$/.test(pin)) throw new Error(`${LEGACY_DECODER_PIN} must hold one SHA-256 without a newline`);
  if (!legacyHash.test(legacy) || sha256(legacy.replace(legacyHash, "")) !== pin)
    throw new Error("R6 bridge changed the legacy generated AST outside its source hash");

  const contract = printedContract();
  const directory = path.dirname(OUTPUT);
  for (const [file, text] of [
    [OUTPUT, legacy],
    [path.join(directory, "printed.rs"), renderPrinted(contract)],
    [path.join(directory, "vocab.rs"), renderVocabulary(contract)],
  ]) {
    const output = path.join(ROOT, file);
    if (check) {
      if (!fs.existsSync(output) || fs.readFileSync(output, "utf8") !== text) {
        console.error(`${file} is stale; run: node scripts/generate-rust-ast.ts`);
        process.exitCode = 1;
      }
    } else {
      fs.writeFileSync(output, text);
      console.log(`wrote ${file} (${text.split("\n").length} lines)`);
    }
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
