// The read-only brief shares typography without loading an editor's state.
import { RichText } from "../../../mtg/index.ts";
import { Eyebrow, Text } from "../../../ui/index.ts";
import styles from "./BriefField.module.css";
export function BriefText({title,value,purpose}:{title:string;value:string;purpose:string}) {
  return <section className={styles.field}><div className={styles.head}><Eyebrow>{title}</Eyebrow></div>
    {value.trim() ? <RichText text={value} /> : <Text tone="muted" as="p" className={styles.empty}>Nothing here yet. {purpose}</Text>}
  </section>;
}
