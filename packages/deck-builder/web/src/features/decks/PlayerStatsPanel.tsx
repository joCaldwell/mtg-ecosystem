import { useVoice } from "../../app/Preferences.tsx";
import type { ReactNode } from "react";
import { api, type PlayerStats } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { CardRef, ColorIdentity, ManaSymbol, COLOR_NAMES } from "../../mtg/index.ts";
import { Box, Chip, EmptyState, ErrorNote, KeyValue, Meter, Section, Text } from "../../ui/index.ts";
import styles from "./PlayerStatsPanel.module.css";

const WUBRG = ["W", "U", "B", "R", "G"] as const;

function Block({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Box>
      <div className={styles.block}>
        <Text tone="muted" size="xs" weight="medium">
          {title}
        </Text>
        {children}
      </div>
    </Box>
  );
}

const num = (n: number | null, digits = 1) => (n == null ? "–" : n.toFixed(digits).replace(/\.0+$/, ""));

function Colors({ stats }: { stats: PlayerStats }) {
  return (
    <Block title="Colours in deck identities">
      <div className={styles.bars}>
        {WUBRG.map((c) => (
          <div key={c} className={styles.bar}>
            <span className={styles.colour}><ManaSymbol symbol={c} /><span>{COLOR_NAMES[c]}</span></span>
            <Meter value={stats.colors[c] ?? 0} max={stats.deck_count} aria-label={`Paper decks including ${COLOR_NAMES[c]}`} />
            <Text mono tone="soft" size="sm">
              {stats.colors[c] ?? 0}
            </Text>
          </div>
        ))}
      </div>
    </Block>
  );
}

/**
 * What the owner builds, across their Paper decks (spec §10.3). The numbers are
 * computed on the server, the same ones an agent gets from `player_stats`;
 * this only arranges them. Empty blocks are left out.
 */
export function PlayerStatsPanel() {
  const voice = useVoice();
  const query = useQuery(() => api.getStats(), []);
  const stats = query.data;
  if (query.error && !stats) return <ErrorNote>{query.error}</ErrorNote>;
  if (!stats) return null;
  if (stats.deck_count === 0) return (
    <Section title="Across your paper decks">
      <EmptyState title="No paper decks yet">
        Mark a deck as Paper using its Actions menu to include it in these statistics.
      </EmptyState>
    </Section>
  );

  const byType: Array<[ReactNode, ReactNode]> = Object.entries(stats.averages.by_type).map(([type, n]) => [
    type,
    <Text mono key={type}>
      {num(n)}
    </Text>,
  ]);
  const averages: Array<[ReactNode, ReactNode]> = [];
  if (stats.averages.mana_value != null) averages.push(["Mana value", <Text mono key="mv">{num(stats.averages.mana_value, 2)}</Text>]);
  if (stats.averages.lands != null) averages.push(["Land count", <Text mono key="l">{num(stats.averages.lands)}</Text>]);

  return (
    <Section title="Across your paper decks" className={styles.stats}>
      <div className={styles.summary}>
        <Text><Text mono weight="bold">{stats.deck_count}</Text> Paper {stats.deck_count === 1 ? "deck" : "decks"} · <Text mono weight="bold">{stats.families}</Text> {stats.families === 1 ? "family" : "families"}</Text>
        <Text tone="muted" size="sm">A fork and its parent count as one family.</Text>
      </div>
      <div className={styles.grid}>
        <Colors stats={stats} />

        {stats.identities.length > 0 && (
          <Block title="Common colour identities">
            <div className={styles.identities}>
              {stats.identities.map((i) => (
                <div key={i.identity} className={styles.row}>
                  <span className={styles.pips}>
                    <ColorIdentity colors={i.identity === "C" ? "" : i.identity} />
                    <span className={styles.identityCode}>{i.identity === "C" ? "Colourless" : [...i.identity].join("/")}</span>
                  </span>
                  <Text mono tone="soft" size="sm">
                    {i.decks} {i.decks === 1 ? "deck" : "decks"}
                  </Text>
                </div>
              ))}
            </div>
          </Block>
        )}

        {stats.commander_types.length > 0 && (
          <Block title="Commander types">
            <div className={styles.chips}>
              {stats.commander_types.map((t) => (
                <Chip key={t.type}>
                  {t.type}
                  <Text mono tone="soft">
                    {t.decks}
                  </Text>
                </Chip>
              ))}
            </div>
          </Block>
        )}

        {averages.length + byType.length > 0 && (
          <Block title="Averages per deck">
            {averages.length > 0 && <KeyValue items={averages} />}
            {byType.length > 0 && (
              <>
                <Text tone="faint" size="xs">
                  Cards of each type
                </Text>
                <div className={styles.typeCounts}>
                  {byType.map(([type, value]) => <div key={String(type)} className={styles.typeCount}><Text>{type}</Text>{value}</div>)}
                </div>
              </>
            )}
          </Block>
        )}

      </div>

      {stats.slots.length > 0 && (
          <Block title="Slots you use">
            <div className={styles.slots}>
              {stats.slots.map((s) => (
                <div key={s.name} className={styles.slot}>
                  <Text className={styles.slotName}>{s.name}</Text>
                  <Text mono tone="soft" size="sm" title="Average cards in this slot, and how many decks use it">
                    avg {num(s.average_count)} · {s.decks} {s.decks === 1 ? "deck" : "decks"}
                  </Text>
                </div>
              ))}
            </div>
          </Block>
        )}
      {stats.reused_cards.length > 0 && (
        <Block title="Cards reused across deck families">
          <ul className={styles.reused}>
            {stats.reused_cards.map((c) => (
              <li key={c.name}>
                <CardRef name={c.name} />
                <Text mono tone="soft" size="sm">
                  {c.decks}
                </Text>
              </li>
            ))}
          </ul>
        </Block>
      )}

      <Text tone="faint" size="xs">
        {voice("Only Paper decks contribute. Mark a deck as Paper to include its counts here.", "Only Paper decks contribute. These are the same computed numbers your agent receives.")}
      </Text>
    </Section>
  );
}
