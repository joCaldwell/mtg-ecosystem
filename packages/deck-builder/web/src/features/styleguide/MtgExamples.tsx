import { api } from "../../api/client.ts";
// Examples of every component in mtg/. Anything that shows a real card is built
// from cards fetched at mount, never typed here: the strings that stay in this
// file are mana-symbol notation, which is not card data.

import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  CardHoverProvider,
  CardImage,
  CardLookupProvider,
  CardOpenProvider,
  CardRef,
  CardSheet,
  ColorIdentity,
  InlineText,
  ManaCost,
  ManaSymbol,
  OracleText,
  Price,
  RichText,
  useCardHover,
  useCardLookup,
  type CardInfo,
} from "../../mtg/index.ts";
import { Button, Row } from "../../ui/index.ts";
import styles from "./MtgExamples.module.css";

const SYMBOLS = ["W", "U", "B", "R", "G", "C", "X", "0", "3", "12", "S", "T"];
const HYBRIDS = ["W/U", "U/B", "B/G", "R/G", "G/W", "2/W", "2/B", "W/P", "B/P", "G/U/P"];

function Example({ title, note, children }: { title: string; note?: ReactNode; children: ReactNode }) {
  return (
    <section className={styles.section}>
      <h3 className={styles.title}>{title}</h3>
      {note && <p className={styles.note}>{note}</p>}
      <div className={styles.body}>{children}</div>
    </section>
  );
}

function Labelled({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className={styles.labelled}>
      <span className={styles.label}>{label}</span>
      {children}
    </div>
  );
}

/** A plain element that names a card, to show useCardHover without CardRef. */
function HoverRow({ card, children }: { card: string | CardInfo; children: ReactNode }) {
  const hover = useCardHover();
  return (
    <div className={styles.row} {...hover(card)}>
      {children}
    </div>
  );
}

/** Primes the lookup cache the way the deck page will with its own cards. */
function Prime({ cards, children }: { cards: CardInfo[]; children: ReactNode }) {
  const { prime } = useCardLookup();
  useEffect(() => prime(cards), [prime, cards]);
  return <>{children}</>;
}

function CardExamples({ cards }: { cards: CardInfo[] }) {
  const [a, b, c] = cards;
  const [opened, setOpened] = useState<string | null>(null);
  const [sheet, setSheet] = useState<"center" | "sheet" | null>(null);
  const markdown = [
    "## A heading",
    `Plain prose with **bold**, *italic*, \`inline code\` and a [link](https://scryfall.com). Hover the reference to see the card: [[${a.name}]].`,
    "",
    "- A bullet that names **[[" + b.name + "]]** inside bold",
    "  - A nested bullet",
    `- Another, with a reason: _${c.name}_ is not a reference, [[${c.name}]] is`,
    "",
    "1. First step",
    "2. Second step",
    "",
    "> A quoted line, for rules text.",
    "",
    "---",
    "```",
    "a fenced block stays literal: {T}, [[not a ref]], **not bold**",
    "```",
  ].join("\n");

  return (
    <>
      <Example
        title="OracleText"
        note="Newlines separate abilities; braced symbols render inline; parenthesised reminder text is muted italic."
      >
        <Labelled label="a fetched card">
          <OracleText text={a.oracle_text} />
        </Labelled>
        <Labelled label="symbols and reminder text">
          <OracleText text={"{2}{U}, {T}: First ability.\nSecond ability on its own line. ({T}: Add {C}. This is reminder text.)"} />
        </Labelled>
        <Labelled label="bare (inside a framed surface)">
          <OracleText text={b.oracle_text} bare />
        </Labelled>
      </Example>

      <Example title="CardSheet" note="Explicit card inspection: tap a card, or focus a CardRef and press ?. Escape closes it and restores focus. Read-only decks open every card this way.">
        <Row gap={4} wrap>
          <Button onClick={() => setSheet("center")}>Open as a dialog</Button>
          <Button size="touch" onClick={() => setSheet("sheet")}>Open as a sheet</Button>
        </Row>
        {sheet && <CardSheet card={a} sheet={sheet === "sheet"} onClose={() => setSheet(null)} />}
      </Example>

      <Example title="CardImage" note="Fixed 488 / 680 proportions. The placeholder keeps the footprint until the picture loads, and when it cannot.">
        <Labelled label="loaded from Scryfall">
          <CardImage name={a.name} width={180} />
        </Labelled>
        <Labelled label="no such card (failure state)">
          <CardImage name="This is not a card name" width={180} />
        </Labelled>
      </Example>

      <Example
        title="Hover card"
        note="Rest the pointer on any element that names a card for half a second. It hides on leave, scroll, mouse down and any key."
      >
        <Labelled label="from a name (looked up)">
          <HoverRow card={b.name}>{b.name}</HoverRow>
        </Labelled>
        <Labelled label="from a CardInfo (immediate)">
          <HoverRow card={c}>{c.name}</HoverRow>
        </Labelled>
      </Example>

      <Example
        title="CardRef"
        note="A card's name inline in text. A host action opens it in context; without one, a click opens CardSheet. Focus a reference and press ? for inspection."
      >
        <CardOpenProvider onOpen={setOpened}>
          <p className={styles.prose}>
            Swapping <CardRef name={a.name} /> for <CardRef name={b.name} card={b} /> keeps the curve; <CardRef name={c.name} onOpen={(n) => setOpened(`${n} (own handler)`)} /> stays.
          </p>
          <p className={styles.note}>{opened ? `Opened: ${opened}` : "Click a reference to open it."}</p>
        </CardOpenProvider>
        <p className={styles.prose}>
          Outside a provider: <CardRef name={a.name} />
        </p>
      </Example>

      <Example title="RichText and InlineText" note="The agent's markdown dialect. Card references inside any of it are real CardRefs.">
        <Labelled label="RichText">
          <RichText text={markdown} />
        </Labelled>
        <Labelled label="InlineText">
          <span className={styles.prose}>
            <InlineText text={`Cut for **mana**: [[${a.name}]] and [[${b.name}]] overlap.`} />
          </span>
        </Labelled>
      </Example>
    </>
  );
}

function usePlayground(): CardInfo[] | null | "error" {
  const [cards, setCards] = useState<CardInfo[] | null | "error">(null);
  useEffect(() => {
    let live = true;
    api.search("t:legendary t:creature mv=4")
      .then((rows: CardInfo[]) => {
        // Alchemy rebalances ("A-" names) have no printed image on Scryfall, so they would only show the placeholder.
        const withArt = rows.filter((r) => !r.name.startsWith("A-"));
        if (live) setCards(withArt.slice(0, 3));
      })
      .catch(() => live && setCards("error"));
    return () => {
      live = false;
    };
  }, []);
  return cards;
}

export function MtgExamples() {
  const fetched = usePlayground();
  const cards = Array.isArray(fetched) && fetched.length === 3 ? fetched : null;

  // The lookup the real app will get from the API: here, a find in the fetched list.
  const lookup = useMemo(
    () => async (name: string) => cards?.find((c) => c.name.toLowerCase() === name.trim().toLowerCase()) ?? null,
    [cards],
  );

  return (
    <CardLookupProvider lookup={lookup}>
      <CardHoverProvider>
        <div className={styles.root}>
          <Example title="ManaSymbol" note="One symbol from inside the braces. Every pip carries its letter or number, so colour is never the only cue; hybrids carry one per half. Black is a dark disc with a pale ring and pale letter, so it never reads as the grey generic pips.">
            <Labelled label="single">
              <div className={styles.strip}>
                {SYMBOLS.map((s) => (
                  <ManaSymbol key={s} symbol={s} />
                ))}
              </div>
            </Labelled>
            <Labelled label="hybrid and Phyrexian">
              <div className={styles.strip}>
                {HYBRIDS.map((s) => (
                  <ManaSymbol key={s} symbol={s} />
                ))}
              </div>
            </Labelled>
            <Labelled label="beside each other: black, generic, colourless">
              <div className={styles.strip}>
                <ManaSymbol symbol="B" />
                <ManaSymbol symbol="2" />
                <ManaSymbol symbol="C" />
                <ManaSymbol symbol="B" />
                <ManaSymbol symbol="1" />
              </div>
            </Labelled>
          </Example>

          <Example title="ManaCost" note="Scryfall's cost string. Two faces are separated by a faint slash; no cost renders nothing.">
            <Labelled label="costs">
              <div className={styles.strip}>
                <ManaCost cost="{2}{U}{B}" />
                <ManaCost cost="{B/G}" />
                <ManaCost cost="{X}{R}" />
                <ManaCost cost="{W/P}{3}{G/U}" />
                <ManaCost cost="{1}{R} // {2}{U}{U}" />
              </div>
            </Labelled>
            <Labelled label="empty">
              <div className={styles.strip}>
                <ManaCost cost={null} />
                <ManaCost cost="" />
                <span className={styles.note}>(nothing above this line)</span>
              </div>
            </Labelled>
          </Example>

          <Example title="Price" note="A card's cheapest printing, from whole cents. In a list, `column` reserves the width so prices line up; no price renders nothing.">
            <Labelled label="inline">
              <div className={styles.strip}>
                <Price cents={5} />
                <Price cents={119} />
                <Price cents={162623} />
                <Price cents={null} />
              </div>
            </Labelled>
            <Labelled label="column">
              <div>
                <div><Price cents={119} column /></div>
                <div><Price cents={null} column /></div>
                <div><Price cents={41237} column /></div>
              </div>
            </Labelled>
          </Example>

          <Example title="ColorIdentity" note="WUBRG order, smaller discs, no letters. Empty means colourless.">
            <div className={styles.strip}>
              <ColorIdentity colors="WUBRG" />
              <ColorIdentity colors="UBR" />
              <ColorIdentity colors="RU" />
              <ColorIdentity colors="G" />
              <ColorIdentity colors="" />
            </div>
          </Example>

          {cards ? (
            <Prime cards={cards}>
              <CardExamples cards={cards} />
            </Prime>
          ) : (
            <p className={styles.note}>
              {fetched === "error"
                ? "Could not load sample cards from /api/search, so the card examples are hidden."
                : fetched === null
                  ? "Loading sample cards…"
                  : "The search returned too few cards to build the examples."}
            </p>
          )}
        </div>
      </CardHoverProvider>
    </CardLookupProvider>
  );
}
