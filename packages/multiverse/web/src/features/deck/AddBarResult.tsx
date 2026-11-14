import { Fragment, type MouseEvent, type ReactNode } from "react";
import type { AnnotatedResult } from "../../domain/search.ts";
import { ManaCost, ManaSymbol, Price, useCardHover } from "../../mtg/index.ts";
import { Button, Chip } from "../../ui/index.ts";
import styles from "./AddBarResult.module.css";

/** The rules text as one line: abilities joined with a dot, symbols drawn. */
function oneLine(text: string): ReactNode[] {
  return text
    .replace(/\n/g, " · ")
    .split(/(\{[^}]+\})/)
    .map((piece, i) =>
      piece.startsWith("{") && piece.endsWith("}") ? (
        <ManaSymbol key={i} symbol={piece.slice(1, -1)} />
      ) : (
        <Fragment key={i}>{piece}</Fragment>
      ),
    );
}

export interface AddBarResultProps {
  result: AnnotatedResult;
  id: string;
  highlighted: boolean;
  /** Offer "as commander": the deck has none and this card can be one. */
  offerCommander: boolean;
  onPick: (shift: boolean) => void;
  onCommander: () => void;
  onHighlight: () => void;
}

export function AddBarResult({ result, id, highlighted, offerCommander, onPick, onCommander, onHighlight }: AddBarResultProps) {
  const hover = useCardHover();
  const { card, inDeck, onMaybe, proposed, lastRejection, memories, legal } = result;
  // The owner's own words beat the card text: they are why it is not here.
  const said = lastRejection?.reason.trim();

  return (
    <div
      id={id}
      role="option"
      aria-selected={highlighted}
      className={`${styles.result} ${highlighted ? styles.highlighted : ""}`}
      onClick={(e: MouseEvent) => onPick(e.shiftKey)}
      onMouseMove={highlighted ? undefined : onHighlight}
      {...hover(card)}
    >
      <span className={styles.head}>
        <span className={styles.name}>{card.name}</span>
        <ManaCost cost={card.mana_cost} />
        <span className={styles.type}>{card.type_line}</span>
        <Price currency={card.currency} cents={card.cents} />
      </span>
      <span className={styles.flags}>
        {!legal && <Chip tone="cut">not legal in Commander</Chip>}
        {inDeck && <Chip>in deck</Chip>}
        {onMaybe && <Chip>maybe list</Chip>}
        {proposed && <Chip tone="agent">proposed</Chip>}
        {memories > 0 && <Chip tone="agent">memory</Chip>}
        {lastRejection && <Chip tone="cut">rejected r{lastRejection.revision}</Chip>}
        {offerCommander && (
          <Button
            size="sm"
            tabIndex={-1}
            onClick={(e) => {
              e.stopPropagation();
              onCommander();
            }}
          >
            as commander
          </Button>
        )}
      </span>
      <span className={said ? styles.said : styles.text}>
        {said ? `You said: “${said}”` : oneLine(card.oracle_text)}
      </span>
      {/* The active result names what Enter and Shift+Enter do. A card already
          here opens instead, so it offers that: drawn as a button, but only a
          picture of the row's own click and Enter (an option may not contain
          a control), so a click on it is the row's. */}
      {highlighted && (inDeck || onMaybe) && (
        <span className={styles.actions} aria-hidden="true">
          <span className={styles.view}>View card</span>
        </span>
      )}
      {highlighted && !inDeck && !onMaybe && (
        <span className={styles.actions}>
          <Button size="sm" tabIndex={-1} onClick={(e) => { e.stopPropagation(); onPick(false); }}>Add</Button>
          <Button size="sm" tabIndex={-1} kbd="⇧↵" onClick={(e) => { e.stopPropagation(); onPick(true); }}>Add to maybe</Button>
        </span>
      )}
    </div>
  );
}
