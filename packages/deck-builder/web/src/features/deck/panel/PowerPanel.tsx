import { Explainer } from "../../../ui/index.ts";
import { useVoice } from "../../../app/Preferences.tsx";
import { useState } from "react";
import { api } from "../../../api/client.ts";
import { useInbox } from "../../../app/Inbox.tsx";
import { bracketLabel, powerRows } from "../../../domain/power.ts";
import { ago } from "../../../domain/time.ts";
import { CardRef, RichText } from "../../../mtg/index.ts";
import { Button, Eyebrow, Row, Stack, Text } from "../../../ui/index.ts";
import { DocumentEditCard } from "../../decisions/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { PowerEditor } from "./PowerEditor.tsx";
import { PowerMapping } from "./PowerMapping.tsx";
import styles from "./PowerPanel.module.css";

export function PowerPanel() {
  const voice = useVoice();
  const state = useDeckState();
  const { power, deck } = state;
  const { deckId, run, apply } = useDeck();
  const inbox = useInbox();
  const [editing, setEditing] = useState<"target" | "assessment" | null>(null);
  const readOnly = deck.category === "archived";
  const a = power.assessment;
  const t = power.target;
  return <>
    <Explainer title="Commander bracket" summary={voice("A bracket helps you describe the game this deck is built for.", "The app maps your or your agent's assessment to a suggested bracket.")}>Set a target for the experience you want, then assess how the current list plays. The app suggests a bracket from those answers and the card data. Unknown answers stay unknown. Discuss the result with your table; it is not a certification.</Explainer>
    <Stack gap={4}>
      <Row justify="between"><Eyebrow>Deck power</Eyebrow><Button size="sm" variant="ghost" disabled={readOnly} onClick={() => setEditing("assessment")}>{a ? "Reassess" : "Assess deck"}</Button></Row>
      <div className={styles.summary}>
        <Stack gap={2}><Text size="xs" tone="muted">Current assessment</Text><Text size="xl" mono tone={a?.stale ? "warn" : "soft"}>{a ? bracketLabel(a.mapping) : "Not assessed"}</Text>
          {a && <Text size="xs" tone={a.owner_touched ? "muted" : "agent"}>{a.author_display ?? a.author} · r{a.revision} · {ago(a.created_at)}</Text>}
        </Stack>
        <Stack gap={2}><Text size="xs" tone="muted">Target</Text><Text size="xl" mono>{t ? bracketLabel(t.mapping) : "Not set"}</Text>
          <Button size="sm" variant="ghost" disabled={readOnly} onClick={() => setEditing("target")}>{t ? "Edit target" : "Set target"}</Button>
          {t && <Text size="xs" tone={t.author_via === "ui" ? "muted" : "agent"}>{t.author_display ?? t.author} · {ago(t.updated_at)}</Text>}
        </Stack>
      </div>
      <Text size="sm" tone="muted">A profile describes how the deck plays. The bracket is a suggested translation for pregame discussion; “?” marks gaps or conflicts.</Text>
      {a?.stale && <Text as="p" tone="warn" role="status">Cards changed since r{a.revision}. This assessment needs review before comparing it with the target.</Text>}
      {a && <Text size="sm" tone="muted">{a.confidence === "tentative" ? "Tentative" : "Supported by evidence"} · {a.evidence === "list" ? "list analysis" : a.evidence === "goldfish" ? "goldfishing" : "actual games"}</Text>}
    </Stack>
    <table className={styles.compare}>
      <caption className={styles.caption}>Power profile comparison</caption>
      <thead><tr><th scope="col">Dimension</th><th scope="col">Target</th><th scope="col">Current{a?.stale ? " (old)" : ""}</th></tr></thead>
      <tbody>{powerRows(power).map(row => <tr key={row.field}><th scope="row">{row.field}</th><td>{row.target}</td><td data-different={power.differences.some(d => d.field === row.field)}>{row.current}</td></tr>)}</tbody>
    </table>
    {power.differences.length > 0 && <Stack gap={3}>
      <Eyebrow>Differences from target</Eyebrow>
      {power.differences.map(d => <Text key={d.field} tone="warn" size="sm" as="p">{d.field}: {d.current}. Target: {d.target}.</Text>)}
    </Stack>}
    {a?.notes && <Stack gap={3}><Eyebrow>Assessment reasoning</Eyebrow><RichText text={a.notes} /></Stack>}
    {t?.notes && <Stack gap={3}><Eyebrow>Target preferences</Eyebrow><RichText text={t.notes} /></Stack>}
    {!!power.game_changers.unknown_cards && <Text as="p" size="sm" tone="warn">{power.game_changers.unknown_cards} card(s) have no Game Changer metadata. Refresh card data in Settings to complete the count.</Text>}
    {!!power.game_changers.cards.length && <Stack gap={2}><Eyebrow>Game Changers in this list</Eyebrow>{power.game_changers.cards.map(c => <CardRef key={c.oracle_id} name={c.name} />)}</Stack>}
    {(t || a) && <details className={styles.details}><summary>Why these brackets?</summary><Stack gap={6}>
      {t && <PowerMapping title="Target" mapping={t.mapping} />}
      {a && <PowerMapping title={a.stale ? "Previous assessment" : "Current assessment"} mapping={a.mapping} />}
      <Text as="p" size="sm" tone="muted">Guidance: <a href={power.guidance.expectations_url} target="_blank" rel="noreferrer">Wizards’ bracket expectations</a> and <a href={power.guidance.url} target="_blank" rel="noreferrer">{power.guidance.version} update</a>. Descriptive dimensions are not averaged into a score.</Text>
    </Stack></details>}
    {state.document_edits.filter(e => e.kind === "power_target").map(edit => <DocumentEditCard key={edit.id} edit={edit} onRuled={r => { apply(r); inbox.apply(r.inbox); }} />)}
    {power.history.length > 1 && <details className={styles.details}><summary>Previous assessments</summary><Stack gap={5}>
      {power.history.slice(1).map(old => <Stack key={old.id} gap={2}>
        <Text size="sm" tone={old.owner_touched ? "muted" : "agent"}>{old.author_display ?? old.author} · r{old.revision} · {bracketLabel(old.mapping)} · {old.confidence} · {old.evidence} · {ago(old.created_at)}</Text>
        <details><summary>View this assessment</summary><Stack gap={2}>{powerRows({ ...power, assessment: old }).map(row => <Text key={row.field} size="sm" tone="muted">{row.field}: {row.current}</Text>)}</Stack></details>
        <RichText text={old.notes} />
      </Stack>)}
    </Stack></details>}
    <Stack gap={3}><Text as="p" size="xs" tone="faint">{readOnly ? "Archived decks are read-only." : voice("Set a target or assess the list above.", "Agents propose target changes for your approval.")} Assessments are attributed estimates; your corrections take precedence.</Text>
      {t && !readOnly && <Button size="sm" variant="ghost" onClick={() => void run(() => api.setPowerTarget(deckId, null))}>Clear target</Button>}
    </Stack>
    {editing && !readOnly && <PowerEditor kind={editing} power={power} revision={deck.revision} onClose={() => setEditing(null)} />}
  </>;
}
