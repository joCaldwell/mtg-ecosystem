// The read-only composition of a deck (spec §18.5, §11.2): the list by slot
// or type, checks, the brief and engines, power, the price and its source, and
// the decisions when there are any. The public page and the phone view are
// both this component: the public page feeds it a shared deck, the phone the
// owner's own deck with its pending proposals drawn in place.
//
// No sidebar, no focus reporting, no hotkeys. A card opens as a sheet on tap.

import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import type { CardInfo } from "../../mtg/index.ts";
import { CardOpenProvider, CardSheet, InlineText, RichText, useCardLookup } from "../../mtg/index.ts";
import { bracketLabel } from "../../domain/power.ts";
import { formatUsd } from "../../domain/price.ts";
import { priceSource, type ReadOnlyDeckModel } from "../../domain/readOnlyDeck.ts";
import { ago } from "../../domain/time.ts";
import { Chip, Eyebrow, Segmented, Stack, Text } from "../../ui/index.ts";
import { useIsPhone } from "../../app/breakpoints.ts";
import { DeckListProvider } from "./DeckStore.tsx";
import { DeckUiProvider, useDeckUi } from "./DeckUi.tsx";
import { DeckList } from "./list/DeckList.tsx";
import { BriefField } from "./panel/BriefField.tsx";
import { ChecksPanel } from "./panel/ChecksPanel.tsx";
import { EngineCard } from "./panel/EngineCard.tsx";
import { DeckViewProvider, useDeckView, type DeckViewModel } from "./useDeckView.ts";
import styles from "./ReadOnlyDeck.module.css";

export interface ReadOnlyDeckProps {
  model: ReadOnlyDeckModel;
  /** Drawn above everything: the public page's art header, the phone's title. */
  header?: ReactNode;
  /** The phone view's ruling controls; given the units it draws in place. */
  rulings?: (view: DeckViewModel) => ReactNode;
}

export function ReadOnlyDeck(props: ReadOnlyDeckProps) {
  return (
    <DeckListProvider state={props.model.list} readOnly>
      <DeckUiProvider>
        <ReadOnlyBody {...props} />
      </DeckUiProvider>
    </DeckListProvider>
  );
}

const NO_SAVE = async () => false;

function ReadOnlyBody({ model, header, rulings }: ReadOnlyDeckProps) {
  const ui = useDeckUi();
  const view = useDeckView({ memories: false });
  const phone = useIsPhone();
  const { lookup } = useCardLookup();
  const [named, setNamed] = useState<CardInfo | null>(null);

  // Pending proposals (the owner's phone only) are drawn into the list as on
  // desktop; a shared deck has none.
  const pending = view.units.length > 0;
  const { reviewing, startReview } = ui;
  useEffect(() => {
    if (pending && !reviewing) startReview();
  }, [pending, reviewing, startReview]);

  const known = useMemo(() => {
    const cards = new Map<string, CardInfo>();
    for (const c of model.list.cards) cards.set(c.oracle_id, c);
    for (const p of model.list.proposals) for (const i of p.items) if (!cards.has(i.oracle_id)) cards.set(i.oracle_id, { ...i, name: i.card_name });
    return cards;
  }, [model.list]);
  const open = ui.openCard ? known.get(ui.openCard.oracleId) ?? null : null;
  const close = () => { ui.openCardPanel(null); setNamed(null); };

  // A card named in prose ([[Card Name]]) opens the same sheet.
  const openByName = useCallback((name: string) => {
    const hit = [...known.values()].find((c) => c.name === name);
    if (hit) return setNamed(hit);
    void lookup(name).then((card) => card && setNamed(card));
  }, [known, lookup]);

  const { power, price } = model;
  return (
    <CardOpenProvider onOpen={openByName}>
      <DeckViewProvider value={view}>
        <div className={styles.root}>
          {header}
          {rulings?.(view)}
          <div className={styles.body}>
            <section className={styles.list} aria-label="Deck list">
              <div className={styles.listHead}>
                <Eyebrow>{model.list.computed.card_count} / 100 cards</Eyebrow>
                <Segmented
                  label="Group by"
                  value={ui.groupBy}
                  onChange={ui.setGroupBy}
                  options={[{ value: "slot", label: "By slot" }, { value: "type", label: "By type" }]}
                />
              </div>
              <DeckList view={view} />
            </section>

            <aside className={styles.aside} aria-label="About this deck">
              <section className={styles.section}>
                <Eyebrow>Estimated price</Eyebrow>
                <Text size="xl" mono as="p">{price.as_of ? formatUsd(price.total_cents) : "No prices yet"}</Text>
                <Text size="xs" tone="muted" as="p">
                  Each card at its cheapest playable printing{price.unpriced ? `; ${price.unpriced} card${price.unpriced === 1 ? "" : "s"} without a price ${price.unpriced === 1 ? "is" : "are"} not counted` : ""}. An estimate, not what any copy is worth.
                </Text>
                {price.as_of && <Text size="xs" tone="faint" as="p">{priceSource(price)} daily estimate</Text>}
              </section>

              <section className={styles.section}>
                <Eyebrow>Power</Eyebrow>
                <div className={styles.power}>
                  <Stack gap={1}>
                    <Text size="xs" tone="muted">Target</Text>
                    <Text size="lg" mono>{power.target ? bracketLabel(power.target.mapping) : "Not set"}</Text>
                    {power.target?.mapping.name && <Text size="xs" tone="soft">{power.target.mapping.name}</Text>}
                  </Stack>
                  <Stack gap={1}>
                    <Text size="xs" tone="muted">Owner's assessment</Text>
                    <Text size="lg" mono tone={power.assessment?.stale ? "warn" : "soft"}>{power.assessment ? bracketLabel(power.assessment.mapping) : "Not assessed"}</Text>
                    {power.assessment && <Text size="xs" tone="muted">{power.assessment.confidence === "tentative" ? "Tentative" : "Supported"} · {ago(power.assessment.created_at)}</Text>}
                  </Stack>
                </div>
                {power.target && power.target.game_changers_max != null && <Text size="sm" tone="muted" as="p">At most {power.target.game_changers_max} Game Changers.</Text>}
                {power.assessment?.stale && <Text size="sm" tone="warn" as="p">The list has changed since this assessment.</Text>}
                {power.target?.notes && <RichText text={power.target.notes} />}
                {power.assessment?.notes && <details className={styles.details}><summary>Assessment reasoning</summary><RichText text={power.assessment.notes} /></details>}
                <Text size="xs" tone="faint" as="p">“B2” is a suggested community bracket for pregame discussion; “?” marks gaps.</Text>
              </section>

              <section className={styles.section}>
                <BriefField readOnly editable={false} title="Thesis" value={model.brief.thesis} purpose="What the deck is trying to do." onSave={NO_SAVE} />
                <BriefField readOnly editable={false} title="Constraints" value={model.brief.constraints_md} purpose="Budget, pod expectations, cards it will not play." onSave={NO_SAVE} />
                <Eyebrow>Engines</Eyebrow>
                {model.engines.length === 0
                  ? <Text tone="muted" size="sm" as="p">No engines named.</Text>
                  : model.engines.map((e) => <EngineCard key={e.name} engine={e} />)}
              </section>

              <section className={styles.section}>
                <Eyebrow>Checks</Eyebrow>
                <ChecksPanel checks={model.checks} />
              </section>

              {model.decisions && (
                <section className={styles.section}>
                  <Eyebrow>Decisions · {model.decisions.length}</Eyebrow>
                  <ol className={styles.decisions}>
                    {model.decisions.map((d, i) => (
                      <li key={`${d.at}-${i}`} className={styles.decision}>
                        <div className={styles.decisionHead}>
                          <Chip tone={d.kind === "accept" ? "add" : d.kind === "reject" ? "cut" : "neutral"}>{d.kind}</Chip>
                          <Text size="xs" tone={d.by === "agent" ? "agent" : "muted"}>{d.who ?? (d.by === "agent" ? "an agent" : "the owner")} · {ago(d.at)}</Text>
                        </div>
                        <Text size="sm" as="p"><InlineText text={d.summary} /></Text>
                        {d.reason && <Text size="sm" tone="soft" as="p">“<InlineText text={d.reason} />”</Text>}
                      </li>
                    ))}
                  </ol>
                </section>
              )}
            </aside>
          </div>
          {(open ?? named) && <CardSheet card={(open ?? named)!} onClose={close} sheet={phone} />}
        </div>
      </DeckViewProvider>
    </CardOpenProvider>
  );
}
