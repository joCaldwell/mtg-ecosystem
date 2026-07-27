// Parser for the supported subset of Scryfall search syntax.
// Grammar:
//   query   := orExpr
//   orExpr  := andExpr ( OR andExpr )*
//   andExpr := unary+
//   unary   := '-' unary | primary
//   primary := '(' orExpr ')' | term
//   term    := '!' name | key op value | name
export type Op = ":" | "=" | "<" | ">" | "<=" | ">=" | "!=";

export type Node =
  | { kind: "and"; children: Node[] }
  | { kind: "or"; children: Node[] }
  | { kind: "not"; child: Node }
  | { kind: "name"; value: string }
  | { kind: "exact"; value: string }
  | { kind: "filter"; key: string; op: Op; value: string };

export class SearchError extends Error {}

type Token =
  | { t: "(" }
  | { t: ")" }
  | { t: "-" }
  | { t: "or" }
  | { t: "term"; node: Node };

const OP_RE = /^(<=|>=|!=|[:=<>])/;
const KEY_RE = /^[a-zA-Z]+/;

function tokenize(input: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;

  const readQuoted = (): string => {
    // caller has consumed the opening quote
    const start = i;
    while (i < input.length && input[i] !== '"') i++;
    if (i >= input.length) throw new SearchError("Unterminated quoted string");
    const s = input.slice(start, i);
    i++; // closing quote
    return s;
  };

  const readWord = (): string => {
    const start = i;
    while (i < input.length && !/[\s()"]/.test(input[i])) i++;
    return input.slice(start, i);
  };

  while (i < input.length) {
    const ch = input[i];
    if (/\s/.test(ch)) {
      i++;
      continue;
    }
    if (ch === "(") {
      tokens.push({ t: "(" });
      i++;
      continue;
    }
    if (ch === ")") {
      tokens.push({ t: ")" });
      i++;
      continue;
    }
    if (ch === "-" && i + 1 < input.length && !/[\s()]/.test(input[i + 1])) {
      tokens.push({ t: "-" });
      i++;
      continue;
    }
    if (ch === "!") {
      i++;
      let value: string;
      if (input[i] === '"') {
        i++;
        value = readQuoted();
      } else {
        value = readWord();
      }
      if (!value) throw new SearchError("Empty exact-name term (!)");
      tokens.push({ t: "term", node: { kind: "exact", value } });
      continue;
    }
    if (ch === '"') {
      i++;
      tokens.push({ t: "term", node: { kind: "name", value: readQuoted() } });
      continue;
    }

    // key<op>value, or a bare name word (possibly the OR operator)
    const rest = input.slice(i);
    const keyMatch = rest.match(KEY_RE);
    if (keyMatch) {
      const afterKey = rest.slice(keyMatch[0].length);
      const opMatch = afterKey.match(OP_RE);
      if (opMatch) {
        const key = keyMatch[0].toLowerCase();
        const op = opMatch[1] as Op;
        i += keyMatch[0].length + op.length;
        let value: string;
        if (input[i] === '"') {
          i++;
          value = readQuoted();
        } else {
          value = readWord();
        }
        if (!value) throw new SearchError(`Missing value for '${key}${op}'`);
        tokens.push({ t: "term", node: { kind: "filter", key, op, value } });
        continue;
      }
    }

    const word = readWord();
    if (word.toLowerCase() === "or") {
      tokens.push({ t: "or" });
    } else {
      tokens.push({ t: "term", node: { kind: "name", value: word } });
    }
  }

  return tokens;
}

export function parse(input: string): Node {
  const tokens = tokenize(input);
  let pos = 0;

  const peek = () => tokens[pos];
  const next = () => tokens[pos++];

  function orExpr(): Node {
    const children = [andExpr()];
    while (peek()?.t === "or") {
      next();
      children.push(andExpr());
    }
    return children.length === 1 ? children[0] : { kind: "or", children };
  }

  function andExpr(): Node {
    const children: Node[] = [];
    while (peek() && peek().t !== ")" && peek().t !== "or") {
      children.push(unary());
    }
    if (!children.length) throw new SearchError("Empty expression");
    return children.length === 1 ? children[0] : { kind: "and", children };
  }

  function unary(): Node {
    if (peek()?.t === "-") {
      next();
      return { kind: "not", child: unary() };
    }
    return primary();
  }

  function primary(): Node {
    const tok = next();
    if (!tok) throw new SearchError("Unexpected end of query");
    if (tok.t === "(") {
      const inner = orExpr();
      if (next()?.t !== ")") throw new SearchError("Missing closing parenthesis");
      return inner;
    }
    if (tok.t === "term") return tok.node;
    throw new SearchError(`Unexpected token '${tok.t}'`);
  }

  if (!tokens.length) throw new SearchError("Empty query");
  const result = orExpr();
  if (pos < tokens.length) throw new SearchError("Unexpected trailing input (unbalanced parentheses?)");
  return result;
}
