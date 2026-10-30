import type { ReactNode } from "react";
import { api, type PlayerStats } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { CardRef, ColorIdentity, ManaSymbol, COLOR_NAMES } from "../../mtg/index.ts";
import { Box, Chip, EmptyState, ErrorNote, Meter, Text } from "../../ui/index.ts";
import styles from "./PlayerStatsPanel.module.css";

const WUBRG = ["W", "U", "B", "R", "G"] as const;

function Block({ title, children, className = "" }: { title: string; children: ReactNode; className?: string }) {
  return <Box className={`${styles.panel} ${className}`}>
    <h3 className={styles.panelTitle}>{title}</h3>
    {children}
  </Box>;
}

const num = (n: number | null, digits = 1) => (n == null ? "–" : n.toFixed(digits).replace(/\.0+$/, ""));

function Colors({ stats }: { stats: PlayerStats }) {
  return <Block title="Colours in deck identities">
    <div className={styles.bars}>
      {WUBRG.map(c => <div key={c} className={styles.bar} data-color={c}>
        <span className={styles.colour}><ManaSymbol symbol={c} /><span>{COLOR_NAMES[c]}</span></span>
        <Meter value={stats.colors[c] ?? 0} max={stats.deck_count} aria-label={`Paper decks including ${COLOR_NAMES[c]}`} />
        <Text mono tone="soft" size="sm">{stats.colors[c] ?? 0} / {stats.deck_count}</Text>
      </div>)}
    </div>
  </Block>;
}

/** Server-computed Paper-deck statistics (§10.3); the gallery only arranges them. */
export function PlayerStatsPanel() {
  const query = useQuery(() => api.getStats(), [], "stats");
  const stats = query.data;
  if (query.error && !stats) return <ErrorNote>{query.error}</ErrorNote>;
  if (!stats) return null;
  if (stats.deck_count === 0) return <section className={styles.stats} aria-labelledby="paper-stats-heading">
    <h2 id="paper-stats-heading" className={styles.heading}>Across your paper decks</h2>
    <EmptyState title="No paper decks yet">Set a deck’s lifecycle to Paper to include its statistics.</EmptyState>
  </section>;

  // stats.ts classifies types by the front face, while computed.land_count
  // includes Land anywhere in the type line (including a back face). Keep
  // the authoritative land metric above; repeating its front-face subset
  // beside it would look like two conflicting counts of the same thing.
  const byType = Object.entries(stats.averages.by_type).filter(([type]) => type !== "Land");
  const figures = [
    ["Paper decks", stats.deck_count],
    ["Families", stats.families],
    ["Mana value", num(stats.averages.mana_value, 2)],
    ["Land count", num(stats.averages.lands)],
  ] as const;

  return <section className={styles.stats} aria-labelledby="paper-stats-heading">
    <h2 id="paper-stats-heading" className={styles.heading}>Across your paper decks</h2>
    <div className={styles.summary}>
      <dl className={styles.figures}>
        {figures.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}
      </dl>
      <Text tone="muted" size="sm">A fork and its parent count as one family.</Text>
    </div>
    <div className={styles.grid}>
      <Colors stats={stats} />
      {(stats.identities.length > 0 || stats.commander_types.length > 0) && <div className={styles.column}>
        {stats.identities.length > 0 && <Block title="Common colour identities">
          <div className={styles.identities}>
            {stats.identities.map(i => <div key={i.identity} className={styles.identity}>
              <span className={styles.pips}><ColorIdentity colors={i.identity === "C" ? "" : i.identity} /></span>
              <Text mono tone="soft" size="sm">{i.decks} {i.decks === 1 ? "deck" : "decks"}</Text>
            </div>)}
          </div>
        </Block>}
        {stats.commander_types.length > 0 && <Block title="Commander types">
          <div className={styles.chips}>
            {stats.commander_types.map(t => <Chip key={t.type}>{t.type}<Text mono tone="soft">{t.decks}</Text></Chip>)}
          </div>
        </Block>}
      </div>}
      {byType.length > 0 && <Block title="Averages per deck" className={styles.averages}>
        <Text tone="muted" size="sm">Cards of each type</Text>
        <div className={styles.typeCounts}>
          {byType.map(([type, value]) => <div key={type} className={styles.typeCount}><Text>{type}</Text><Text mono tone="soft">{num(value)}</Text></div>)}
        </div>
      </Block>}
    </div>
    {stats.slots.length > 0 && <Block title="Slots you use">
      <div className={styles.slots}>
        {stats.slots.map(s => <div key={s.name} className={styles.slot}>
          <Text className={styles.slotName}>{s.name}</Text>
          <Text mono tone="soft" size="sm">avg {num(s.average_count)} · {s.decks} {s.decks === 1 ? "deck" : "decks"}</Text>
        </div>)}
      </div>
    </Block>}
    {stats.reused_cards.length > 0 && <Block title="Cards reused across deck families">
      <ul className={styles.reused}>
        {stats.reused_cards.map(c => <li key={c.name}><CardRef name={c.name} /><Text mono tone="soft" size="sm">{c.decks}</Text></li>)}
      </ul>
    </Block>}
  </section>;
}
