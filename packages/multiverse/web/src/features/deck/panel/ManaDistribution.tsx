import type { ColourDistribution } from "../../../domain/deckStats.ts";
import { STAT_COLOURS } from "../../../domain/deckStats.ts";
import styles from "./ManaDistribution.module.css";

export interface ManaSeries { title: string; unit: string; centerUnit: string; distribution: ColourDistribution; empty: string }

/** One donut with its caption. The counts it draws are in `ManaLegend`. */
function ManaDonut({ title, unit, centerUnit, distribution, empty }: ManaSeries) {
  return <figure className={styles.figure}>
    <figcaption className={styles.caption}>
      <h2>{title}</h2>
      <span>{distribution.total} {unit}</span>
    </figcaption>
    <svg className={styles.donut} viewBox="0 0 112 112" aria-hidden="true">
      <circle className={styles.track} cx="56" cy="56" r="44" fill="none" strokeWidth="16" />
      <g transform="rotate(-90 56 56)">
        {distribution.rows.filter(row => row.count > 0).map(row => <g key={row.id}>
          {row.id === "B" && <circle className={styles.blackEdge} cx="56" cy="56" r="44" fill="none" strokeWidth="18" pathLength="100" strokeDasharray={`${row.share * 100} ${100 - row.share * 100}`} strokeDashoffset={-row.offset * 100} />}
          <circle className={`${styles.slice} ${styles[row.id]}`} cx="56" cy="56" r="44" fill="none" strokeWidth="16" pathLength="100" strokeDasharray={`${row.share * 100} ${100 - row.share * 100}`} strokeDashoffset={-row.offset * 100} />
        </g>)}
      </g>
      <text x="56" y="53" textAnchor="middle" className={styles.total}>{distribution.total}</text>
      <text x="56" y="70" textAnchor="middle" className={styles.unit}>{centerUnit}</text>
    </svg>
    {!distribution.total && <p className={styles.empty}>{empty}</p>}
  </figure>;
}

/** Two donuts side by side over one table: a row per colour, a count and share per series. */
export function ManaDistribution({ series }: { series: [ManaSeries, ManaSeries] }) {
  return <div className={styles.distribution}>
    <div className={styles.donuts}>{series.map(s => <ManaDonut key={s.title} {...s} />)}</div>
    <table className={styles.legend}>
      <thead><tr><th scope="col"><span className="visually-hidden">Colour</span></th>
        {series.map(s => <th key={s.title} scope="colgroup" colSpan={2}>{s.title}</th>)}</tr></thead>
      <tbody>{STAT_COLOURS.map(({ id, name }) => <tr key={id}>
        <th scope="row"><span className={styles.colour}><i className={styles[id]} aria-hidden="true" />{name}</span></th>
        {series.map(s => {
          const row = s.distribution.rows.find(r => r.id === id)!;
          return [<td key={`${s.title}-n`} className={styles.count}>{row.count}</td>,
            <td key={`${s.title}-p`} className={styles.share}>{row.percentage}</td>];
        })}
      </tr>)}</tbody>
    </table>
  </div>;
}
