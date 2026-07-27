import { type Node, type Op, SearchError } from "./parse.ts";

export interface Compiled {
  sql: string;
  params: (string | number)[];
}

export interface CompileContext {
  deckId?: number;
}

const COLOR_BIT: Record<string, number> = { w: 1, u: 2, b: 4, r: 8, g: 16 };

// Guild/shard/wedge/college/four-color nicknames, per the Scryfall docs.
const COLOR_NICKNAMES: Record<string, string> = {
  white: "w", blue: "u", black: "b", red: "r", green: "g",
  azorius: "wu", dimir: "ub", rakdos: "br", gruul: "rg", selesnya: "gw",
  orzhov: "wb", izzet: "ur", golgari: "bg", boros: "rw", simic: "gu",
  bant: "gwu", esper: "wub", grixis: "ubr", jund: "brg", naya: "rgw",
  abzan: "wbg", jeskai: "urw", sultai: "bgu", mardu: "rwb", temur: "gur",
  silverquill: "wb", prismari: "ur", witherbloom: "bg", lorehold: "rw", quandrix: "gu",
  artifice: "wubr", chaos: "ubrg", aggression: "wbrg", altruism: "wurg", growth: "wubg",
  rainbow: "wubrg", wubrg: "wubrg",
};

type ColorValue = { type: "mask"; mask: number } | { type: "count"; n: number } | { type: "multicolor" };

function parseColorValue(raw: string): ColorValue {
  const v = raw.toLowerCase();
  if (/^\d+$/.test(v)) return { type: "count", n: Number(v) };
  if (v === "m" || v === "multicolor") return { type: "multicolor" };
  if (v === "c" || v === "colorless") return { type: "mask", mask: 0 };
  const letters = COLOR_NICKNAMES[v] ?? v;
  let mask = 0;
  for (const ch of letters) {
    const bit = COLOR_BIT[ch];
    if (bit === undefined)
      throw new SearchError(
        `Unrecognized color value '${raw}' (use wubrg letters, color names, or set nicknames)`,
      );
    mask |= bit;
  }
  return { type: "mask", mask };
}

function escapeLike(s: string): string {
  return s.replace(/[\\%_]/g, (c) => "\\" + c);
}

function likeContains(column: string, value: string): Compiled {
  return { sql: `${column} LIKE ? ESCAPE '\\'`, params: [`%${escapeLike(value)}%`] };
}

// o:/fo: text search. `~` in the pattern is a placeholder for the card's
// own name, so the pattern is built with per-row concatenation.
function oracleSearch(column: string, value: string): Compiled {
  if (!value.includes("~")) return likeContains(column, value);
  const parts = value.split("~").map(escapeLike);
  const exprs: string[] = ["'%'"];
  const params: (string | number)[] = [];
  parts.forEach((part, idx) => {
    if (idx > 0) exprs.push("name");
    if (part) {
      exprs.push("?");
      params.push(part);
    }
  });
  exprs.push("'%'");
  return { sql: `${column} LIKE (${exprs.join(" || ")}) ESCAPE '\\'`, params };
}

// Set-comparison algebra over color bitmasks.
// ':' means "at least" for c: and "within" for id: — per Scryfall's docs.
function maskCompare(column: string, countColumn: string, op: Op, value: ColorValue, defaultOp: "<=" | ">="): Compiled {
  if (value.type === "count") {
    const sqlOp = op === ":" ? "=" : op;
    return { sql: `${countColumn} ${sqlOp} ?`, params: [value.n] };
  }
  if (value.type === "multicolor") {
    if (op === ":" || op === "=") return { sql: `${countColumn} >= 2`, params: [] };
    throw new SearchError(`Comparator '${op}' is not supported with 'multicolor'`);
  }

  const m = value.mask;
  const effOp: Op = op === ":" ? (m === 0 ? "=" : defaultOp) : op;
  const subset = `(${column} & ~${m}) = 0`;
  const superset = `(${column} & ${m}) = ${m}`;
  const equal = `${column} = ${m}`;
  switch (effOp) {
    case "=": return { sql: equal, params: [] };
    case "!=": return { sql: `${column} != ${m}`, params: [] };
    case "<=": return { sql: subset, params: [] };
    case "<": return { sql: `(${subset} AND ${column} != ${m})`, params: [] };
    case ">=": return { sql: superset, params: [] };
    case ">": return { sql: `(${superset} AND ${column} != ${m})`, params: [] };
    default: throw new SearchError(`Unsupported color comparator '${op}'`);
  }
}

const STAT_COLUMNS: Record<string, string> = {
  pow: "power_num", power: "power_num",
  tou: "toughness_num", toughness: "toughness_num",
  loy: "loyalty_num", loyalty: "loyalty_num",
};

function numericCompare(column: string, op: Op, value: string): Compiled {
  const sqlOp = op === ":" ? "=" : op;
  // pow>tou style: right side may be another stat
  const rightCol = STAT_COLUMNS[value.toLowerCase()];
  if (rightCol) {
    return { sql: `${column} IS NOT NULL AND ${rightCol} IS NOT NULL AND ${column} ${sqlOp} ${rightCol}`, params: [] };
  }
  if (!/^[+-]?\d+(\.\d+)?$/.test(value))
    throw new SearchError(`Expected a number, got '${value}'`);
  return { sql: `${column} IS NOT NULL AND ${column} ${sqlOp} ?`, params: [Number(value)] };
}

function compileFilter(key: string, op: Op, value: string, ctx: CompileContext): Compiled {
  switch (key) {
    case "t":
    case "type":
      return likeContains("type_line", value);

    case "o":
    case "oracle":
      return oracleSearch("search_text", value);
    case "fo":
    case "fulloracle":
      return oracleSearch("full_search_text", value);

    case "cmc":
    case "mv":
    case "manavalue": {
      const v = value.toLowerCase();
      if (v === "even" || v === "odd") {
        if (op !== ":" && op !== "=")
          throw new SearchError(`'${key}${op}${value}' is not supported; use ${key}:${v}`);
        return { sql: `CAST(cmc AS INTEGER) % 2 = ${v === "even" ? 0 : 1}`, params: [] };
      }
      if (!/^\d+(\.\d+)?$/.test(v)) throw new SearchError(`Expected a number for '${key}', got '${value}'`);
      const sqlOp = op === ":" ? "=" : op;
      return { sql: `cmc ${sqlOp} ?`, params: [Number(v)] };
    }

    case "c":
    case "color":
      return maskCompare("colors_mask", "colors_count", op, parseColorValue(value), ">=");
    case "id":
    case "identity":
      return maskCompare("ci_mask", "ci_count", op, parseColorValue(value), "<=");

    case "pow":
    case "power":
    case "tou":
    case "toughness":
    case "loy":
    case "loyalty":
      return numericCompare(STAT_COLUMNS[key], op, value);

    case "is": {
      const v = value.toLowerCase();
      if (v === "commander") return { sql: "is_commander = 1", params: [] };
      throw new SearchError(`'is:${value}' is not supported (supported: is:commander)`);
    }

    case "f":
    case "format": {
      if (value.toLowerCase() !== "commander")
        throw new SearchError("Only f:commander is supported");
      return { sql: "commander_legality = 'legal'", params: [] };
    }
    case "banned": {
      if (value.toLowerCase() !== "commander")
        throw new SearchError("Only banned:commander is supported");
      return { sql: "commander_legality = 'banned'", params: [] };
    }

    case "slot": {
      if (ctx.deckId === undefined)
        throw new SearchError("'slot:' filters require a deck context");
      if (value.toLowerCase() === "none")
        return {
          sql: "oracle_id IN (SELECT oracle_id FROM deck_cards WHERE deck_id = ? AND slot_id IS NULL AND role != 'companion')",
          params: [ctx.deckId],
        };
      return {
        sql: `oracle_id IN (
          SELECT dc.oracle_id FROM deck_cards dc JOIN slots s ON s.id = dc.slot_id
          WHERE dc.deck_id = ? AND s.name = ? COLLATE NOCASE)`,
        params: [ctx.deckId, value],
      };
    }
    case "tag": {
      if (ctx.deckId === undefined)
        throw new SearchError("'tag:' filters require a deck context");
      return {
        sql: `oracle_id IN (
          SELECT dct.oracle_id FROM deck_card_tags dct JOIN tags t ON t.id = dct.tag_id
          WHERE dct.deck_id = ? AND t.name = ? COLLATE NOCASE)`,
        params: [ctx.deckId, value],
      };
    }

    default:
      throw new SearchError(
        `Unsupported search key '${key}'. Supported: t, o, fo, cmc/mv, c, id, pow, tou, loy, is, f, banned`,
      );
  }
}

export function compile(node: Node, ctx: CompileContext = {}): Compiled {
  switch (node.kind) {
    case "and": {
      const parts = node.children.map((n) => compile(n, ctx));
      return {
        sql: `(${parts.map((p) => p.sql).join(" AND ")})`,
        params: parts.flatMap((p) => p.params),
      };
    }
    case "or": {
      const parts = node.children.map((n) => compile(n, ctx));
      return {
        sql: `(${parts.map((p) => p.sql).join(" OR ")})`,
        params: parts.flatMap((p) => p.params),
      };
    }
    case "not": {
      const inner = compile(node.child, ctx);
      return { sql: `NOT ${inner.sql}`, params: inner.params };
    }
    case "name":
      return likeContains("name", node.value);
    case "exact":
      return {
        sql: "oracle_id IN (SELECT oracle_id FROM card_names WHERE name_norm = ?)",
        params: [node.value.trim().toLowerCase().normalize("NFC")],
      };
    case "filter":
      return compileFilter(node.key, node.op, node.value, ctx);
  }
}
