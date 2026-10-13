import { useEffect, useState } from "react";
import { api } from "../../api/client.ts";
import type { TableCard, TableObject } from "../../api/client.ts";
import {
  CardInspection,
  PlayerSummary,
  TableZone,
  TextCard,
} from "../../mtg/index.ts";
import { Button, CounterControl, Section } from "../../ui/index.ts";
import styles from "./TableExamples.module.css";
export function TableExamples() {
  const [card, setCard] = useState<TableCard | null>(null),
    [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState(false),
    [tapped, setTapped] = useState(false),
    [life, setLife] = useState(40),
    [counter, setCounter] = useState(2);
  useEffect(() => {
    let alive = true;
    void api
      .search("t:creature", undefined, false)
      .then((cards) => (cards[0] ? api.tableCard(cards[0].oracle_id) : null))
      .then((r) => {
        if (alive) setCard(r?.card ?? null);
      })
      .catch((e) => {
        if (alive) setError(e.message);
      });
    return () => {
      alive = false;
    };
  }, []);
  const obj: TableObject | null = card
    ? {
        id: "sample",
        oracle_id: card.oracle_id,
        role: "card",
        token: true,
        face: 0,
        tapped,
        row: "creatures",
        counters: { "+1/+1": counter },
        power: null,
        toughness: null,
        attached_to: null,
        cast_count: 0,
      }
    : null;
  return (
    <Section
      title="Table components"
      aside="Rules at 15px · text cards, zones and contextual tools shared with the future game interface"
    >
      <div className={styles.examples}>
        <CounterControl
          label="Example counters"
          value={counter}
          onChange={setCounter}
        />
        <CounterControl
          label="Disabled counters"
          value={counter}
          onChange={setCounter}
          disabled
        />
      </div>
      {error && <p>{error}</p>}
      {!card || !obj ? (
        <p>Card data is required to display verified examples.</p>
      ) : (
        <>
          <div className={styles.examples}>
            <TextCard
              observation={{ visibility: "known", card, object: obj }}
              full
              selected={selected}
              onSelect={() => setSelected((v) => !v)}
              footer={
                <Button
                  aria-pressed={tapped}
                  onClick={() => setTapped((v) => !v)}
                >
                  {tapped ? "Untap" : "Tap"}
                </Button>
              }
            />
            <TextCard
              observation={{ visibility: "known", card, object: obj }}
              targeted
            />
            <TextCard observation={{ visibility: "hidden" }} />
            <TextCard
              observation={{ visibility: "known", card, object: obj }}
              disabled
              disabledReason="Target choice pending"
            />
            <div>
              <PlayerSummary
                name="You"
                life={life}
                turn={3}
                status="Active player"
                onLifeChange={setLife}
              />
            </div>
          </div>
          <TableZone label="Creatures" showHeader={false} count={1}>
            <TextCard
              observation={{ visibility: "known", card, object: obj }}
            />
          </TableZone>
          <CardInspection
            observation={{ visibility: "known", card }}
            context="Pinned selection"
          >
            <p>The host supplies optional contextual tools here.</p>
          </CardInspection>
          <h3>Future four-player study</h3>
          <p>
            Presentation only. Hidden-hand observations contain no card
            identity.
          </p>
          <div className={styles.players}>
            {[0, 1, 2, 3].map((i) => (
              <section key={i}>
                <PlayerSummary
                  name={i ? `Player ${i + 1}` : "You"}
                  life={40}
                  status={
                    i === 0
                      ? "Active player"
                      : i === 1
                        ? "Has priority"
                        : "Waiting"
                  }
                />
                <TableZone label="Battlefield" count={1} showHeader={false}>
                  <TextCard
                    observation={{
                      visibility: "known",
                      card,
                      object: obj,
                      owner: i === 2 ? "Player 4" : `Player ${i + 1}`,
                      controller: `Player ${i + 1}`,
                    }}
                    targeted={i === 2}
                  />
                </TableZone>
                <TableZone label="Hand" count={3}>
                  {i ? (
                    <>
                      <TextCard observation={{ visibility: "hidden" }} />
                      <TextCard observation={{ visibility: "hidden" }} />
                      <TextCard observation={{ visibility: "hidden" }} />
                    </>
                  ) : (
                    <TextCard
                      observation={{ visibility: "known", card }}
                      full
                    />
                  )}
                </TableZone>
              </section>
            ))}
          </div>
          <TableZone label="Shared stack" count={1}>
            <TextCard
              observation={{
                visibility: "known",
                card,
                controller: "Player 2",
              }}
              full
            />
            <Button disabled title="Finish choosing a target first">
              Pass priority
            </Button>
          </TableZone>
        </>
      )}
    </Section>
  );
}
