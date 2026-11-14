import type { DeckState } from "../../../api/client.ts";
import { ManaCost } from "../../../mtg/index.ts";
import styles from "./ManaFaceTable.module.css";

type Face = DeckState["computed"]["mana"]["first_face"];
const COLOURS = [["W", "White"], ["U", "Blue"], ["B", "Black"], ["R", "Red"], ["G", "Green"]] as const;
export function ManaFaceTable({ face }: { face: Face }) {
  return <table className={styles.table}>
    <thead><tr><th rowSpan={2} scope="col">Colour</th><th rowSpan={2} scope="col">Fixed cost<br />symbols</th><th colSpan={2} scope="colgroup">Recognized source copies</th></tr>
      <tr><th scope="col">Lands</th><th scope="col">Other permanents</th></tr></thead>
    <tbody>{COLOURS.map(([color, label]) => <tr key={color}>
      <th scope="row"><span className={styles.color}><ManaCost cost={`{${color}}`} />{label}</span></th>
      <td>{face.costs.fixed_symbols[color]}</td><td>{face.sources.land.by_type[color]}</td><td>{face.sources.nonland.by_type[color]}</td>
    </tr>)}</tbody>
  </table>;
}
