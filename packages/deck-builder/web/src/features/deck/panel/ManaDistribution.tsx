import type { ColourDistribution } from "../../../domain/deckStats.ts";
import styles from "./ManaDistribution.module.css";

export function ManaDistribution({ title, unit, centerUnit, distribution, empty }: {
  title: string; unit: string; centerUnit: string; distribution: ColourDistribution; empty: string;
}) {
  return <figure className={styles.figure} aria-label={title}>
    <figcaption className={styles.caption}>
      <h2>{title}</h2>
      <span>{distribution.total} {unit}</span>
    </figcaption>
    <div className={styles.chart}>
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
      <ul className={styles.legend} aria-label={`${title}: count and share`}>
        {distribution.rows.map(row => <li key={row.id} aria-label={`${row.name}: ${row.count}; ${distribution.total ? `${row.percentage} share` : "no percentage"}`}>
          <span className={styles.colour}><i className={styles[row.id]} aria-hidden="true" />{row.name}</span>
          <b>{row.count}</b><span className={styles.share}>{row.percentage}</span>
        </li>)}
      </ul>
    </div>
    {!distribution.total && <p className={styles.empty}>{empty}</p>}
  </figure>;
}
