// The agent writes markdown — bold for quoted rules text, numbered lists for
// the swaps it is proposing, the occasional heading — and until now all of it
// arrived as literal asterisks. This renders it.
//
// Hand-written rather than a dependency, for one reason that outranks size:
// [[Card Name]] has to survive the trip. A library that emits HTML strings
// would force dangerouslySetInnerHTML, and the card chips would lose the click
// handler that makes them peekable. Parsing to React nodes keeps them real
// components, nested anywhere the agent puts them — inside bold, inside a list
// item, inside a link.
//
// The dialect is what the agent actually emits, not all of CommonMark: bold,
// italic, inline code, links, headings, bullet and numbered lists (nested),
// blockquotes, fenced code, and rules. Anything unrecognised stays literal
// text, which is the right failure mode — nothing disappears.

import { Fragment, type ReactNode } from "react";
import { usePeekProps } from "./CardPeek.tsx";

// ---------- inline ----------

function CardRef({ name }: { name: string }) {
  const peekProps = usePeekProps();
  return (
    <span className="cardref" {...peekProps(name)}>
      {name}
    </span>
  );
}

/**
 * Emphasis, code, links and card refs, nested. Scanned character by character
 * rather than by one master regex: the delimiters can contain each other, and
 * an unclosed one has to fall back to a literal instead of eating the rest of
 * the message.
 */
export function inline(text: string): ReactNode {
  const out: ReactNode[] = [];
  let buf = "";
  let k = 0;
  const push = (node: ReactNode) => {
    if (buf) {
      out.push(<Fragment key={k++}>{buf}</Fragment>);
      buf = "";
    }
    out.push(<Fragment key={k++}>{node}</Fragment>);
  };

  let i = 0;
  while (i < text.length) {
    const c = text[i];

    // [[Card Name]] — checked before the link rule, which also starts with [.
    if (c === "[" && text[i + 1] === "[") {
      const end = text.indexOf("]]", i + 2);
      if (end > i + 2) {
        push(<CardRef name={text.slice(i + 2, end)} />);
        i = end + 2;
        continue;
      }
    }

    if (c === "[") {
      const m = /^\[([^\]]*)\]\(([^)\s]+)\)/.exec(text.slice(i));
      if (m) {
        push(
          <a href={m[2]} target="_blank" rel="noreferrer">
            {inline(m[1])}
          </a>,
        );
        i += m[0].length;
        continue;
      }
    }

    if (c === "`") {
      const end = text.indexOf("`", i + 1);
      if (end > i + 1) {
        // Code spans are literal all the way down — no nested parse.
        push(<code className="md-code-span">{text.slice(i + 1, end)}</code>);
        i = end + 1;
        continue;
      }
    }

    if (c === "*" && text[i + 1] === "*") {
      const end = text.indexOf("**", i + 2);
      if (end > i + 2) {
        push(<strong>{inline(text.slice(i + 2, end))}</strong>);
        i = end + 2;
        continue;
      }
    }

    // Single-delimiter emphasis. The closing mark has to be on the same line,
    // and `_` additionally has to sit on a word boundary — oracle_id and
    // card_name are ordinary words in this app, not italics.
    if ((c === "*" || c === "_") && !/[\s*_]/.test(text[i + 1] ?? "")) {
      const wordChar = /[\p{L}\p{N}]/u;
      const opensWord = c === "*" || !wordChar.test(text[i - 1] ?? "");
      if (opensWord) {
        const rest = text.slice(i + 1);
        const stop = rest.search(c === "*" ? /\*|\n/ : /_|\n/);
        const closesWord =
          stop > 0 && rest[stop] === c && (c === "*" || !wordChar.test(rest[stop + 1] ?? ""));
        if (closesWord) {
          push(<em>{inline(rest.slice(0, stop))}</em>);
          i += stop + 2;
          continue;
        }
      }
    }

    buf += c;
    i++;
  }
  if (buf) out.push(<Fragment key={k++}>{buf}</Fragment>);
  return out;
}

/** Inline markdown only — for one-line surfaces like an audit finding title. */
export function CardText({ text }: { text: string }) {
  return <>{inline(text)}</>;
}

// ---------- blocks ----------

const FENCE = /^\s*```(\w*)\s*$/;
const HEADING = /^(#{1,6})\s+(.*)$/;
const RULE = /^\s*([-*_])\s*(?:\1\s*){2,}$/;
const QUOTE = /^\s*>/;
const ITEM = /^(\s*)([-*+]|\d{1,9}[.)])(\s+)(.*)$/;

/**
 * True for a line that opens a new block, so a paragraph knows to stop. The
 * agent routinely drops straight from a sentence into a list with no blank
 * line between them.
 */
function startsBlock(line: string) {
  return (
    FENCE.test(line) ||
    HEADING.test(line) ||
    RULE.test(line) ||
    QUOTE.test(line) ||
    ITEM.test(line)
  );
}

/**
 * A list item's contents. Tight items — no blank line inside — keep their own
 * text unwrapped, so a bullet with a sub-list under it doesn't get a paragraph
 * gap opened between the two.
 */
function listItem(body: string[]): ReactNode {
  if (body.length === 1) return inline(body[0]);
  const cut = body.findIndex(startsBlock);
  if (cut !== 0 && !body.some((l) => !l.trim())) {
    const lead = cut < 0 ? body : body.slice(0, cut);
    return (
      <>
        {lead.map((l, n) => (
          <Fragment key={n}>
            {n > 0 && <br />}
            {inline(l)}
          </Fragment>
        ))}
        {cut > 0 && parseBlocks(body.slice(cut))}
      </>
    );
  }
  return parseBlocks(body);
}

function parseBlocks(lines: string[]): ReactNode[] {
  const out: ReactNode[] = [];
  let i = 0;
  let k = 0;

  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i++;
      continue;
    }

    const fence = FENCE.exec(line);
    if (fence) {
      const body: string[] = [];
      i++;
      while (i < lines.length && !FENCE.test(lines[i])) body.push(lines[i++]);
      // An unterminated fence runs to the end rather than dropping the text.
      if (i < lines.length) i++;
      out.push(
        <pre key={k++} className="md-code">
          <code>{body.join("\n")}</code>
        </pre>,
      );
      continue;
    }

    // Checked before ITEM: "*** " is a rule, not a bullet.
    if (RULE.test(line)) {
      out.push(<hr key={k++} className="md-hr" />);
      i++;
      continue;
    }

    const heading = HEADING.exec(line);
    if (heading) {
      // Levels collapse at 3: these render inside a chat bubble or a finding
      // row, where a six-step type scale has nothing to be relative to.
      out.push(
        <div key={k++} className={`md-h md-h${Math.min(heading[1].length, 3)}`}>
          {inline(heading[2])}
        </div>,
      );
      i++;
      continue;
    }

    if (QUOTE.test(line)) {
      const body: string[] = [];
      while (i < lines.length && (QUOTE.test(lines[i]) || (body.length && lines[i].trim())))
        body.push(lines[i++].replace(/^\s*>\s?/, ""));
      out.push(
        <blockquote key={k++} className="md-quote">
          {parseBlocks(body)}
        </blockquote>,
      );
      continue;
    }

    const first = ITEM.exec(line);
    if (first) {
      const ordered = /\d/.test(first[2]);
      const indent = first[1].length;
      const items: string[][] = [];
      while (i < lines.length) {
        const m = ITEM.exec(lines[i]);
        if (m && m[1].length === indent && /\d/.test(m[2]) === ordered) {
          items.push([m[4]]);
          i++;
          // Continuation and nested lines: anything indented past this
          // marker, plus blank lines that turn out to be interior.
          const contentCol = m[1].length + m[2].length + m[3].length;
          while (i < lines.length) {
            const next = lines[i];
            if (!next.trim()) {
              // Only interior if something indented follows it.
              const after = lines[i + 1];
              if (after?.trim() && after.search(/\S/) >= contentCol) {
                items[items.length - 1].push("");
                i++;
                continue;
              }
              break;
            }
            if (next.search(/\S/) < contentCol) break;
            items[items.length - 1].push(next.slice(contentCol));
            i++;
          }
          continue;
        }
        // A blank line between items keeps the list going; anything else ends it.
        if (!lines[i].trim() && ITEM.exec(lines[i + 1] ?? "")) {
          i++;
          continue;
        }
        break;
      }

      const rendered = items.map((body, n) => <li key={n}>{listItem(body)}</li>);
      out.push(
        ordered ? (
          <ol key={k++} className="md-list" start={Number(first[2].replace(/\D/g, "")) || 1}>
            {rendered}
          </ol>
        ) : (
          <ul key={k++} className="md-list">
            {rendered}
          </ul>
        ),
      );
      continue;
    }

    const body: string[] = [];
    while (i < lines.length && lines[i].trim() && !startsBlock(lines[i])) body.push(lines[i++]);
    out.push(
      <p key={k++} className="md-p">
        {/* A soft line break stays a line break. The agent uses them to keep
            clauses apart, and reflowing them into one run loses that. */}
        {body.map((l, n) => (
          <Fragment key={n}>
            {n > 0 && <br />}
            {inline(l)}
          </Fragment>
        ))}
      </p>,
    );
  }

  return out;
}

/** Block-level markdown: agent messages, audit details, reasoning summaries. */
export function Markdown({ text, className = "" }: { text: string; className?: string }) {
  return <div className={`md ${className}`}>{parseBlocks(text.replace(/\r\n?/g, "\n").split("\n"))}</div>;
}
