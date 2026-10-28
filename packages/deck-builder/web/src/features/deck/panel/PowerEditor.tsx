import { useVoice } from "../../../app/Preferences.tsx";
import { useState, type FormEvent } from "react";
import { api, type Power, type PowerAssessmentInput, type PowerProfile } from "../../../api/client.ts";
import { emptyPowerProfile } from "../../../domain/power.ts";
import { Button, Dialog, ErrorNote, Field, Row, Select, Text, TextArea, TextField } from "../../../ui/index.ts";
import { useDeck } from "../DeckStore.tsx";
import styles from "./PowerPanel.module.css";

export function PowerEditor({ kind, power, revision, onClose }: {
  kind: "target" | "assessment"; power: Power; revision: number; onClose: () => void;
}) {
  const voice = useVoice();
  const { deckId, run, error } = useDeck();
  const target = kind === "target";
  const source = target ? power.target : power.assessment;
  const [profile, setProfile] = useState<PowerProfile>(source?.profile ?? emptyPowerProfile());
  const [first, setFirst] = useState(source?.profile.win_turn?.min.toString() ?? "");
  const [last, setLast] = useState(source?.profile.win_turn?.max.toString() ?? "");
  const [early, setEarly] = useState(source?.profile.early_win_turn?.toString() ?? "");
  const [limit, setLimit] = useState(power.target?.game_changers_max?.toString() ?? "");
  const [notes, setNotes] = useState(source?.notes ?? "");
  const [evidence, setEvidence] = useState<PowerAssessmentInput["evidence"]>(target ? "list" : power.assessment?.evidence ?? "list");
  const [confidence, setConfidence] = useState<PowerAssessmentInput["confidence"]>(target ? "tentative" : power.assessment?.confidence ?? "tentative");
  const [busy, setBusy] = useState(false);
  // Capture the list being edited. Live updates must not relabel an old form
  // as an assessment of a new revision; the service rejects stale submissions.
  const [assessedRevision] = useState(revision);

  async function save(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    const value = { ...profile, win_turn: first || last ? { min: Number(first), max: Number(last) } : null, early_win_turn: early ? Number(early) : null };
    const result = await run(() => target
      ? api.setPowerTarget(deckId, { profile: value, game_changers_max: limit ? Number(limit) : null, notes })
      : api.assessPower(deckId, { profile: value, revision: assessedRevision, evidence, confidence, notes }));
    setBusy(false);
    if (result) onClose();
  }
  const fields = power.fields;
  return <Dialog title={target ? "Set the power target" : "Assess the current deck"} width="lg" onClose={onClose} footer={<>
    <Button onClick={onClose} disabled={busy}>Cancel</Button>
    <Button variant="primary" type="submit" form="power-editor" disabled={busy}>{busy ? "Saving…" : target ? "Save target" : "Save assessment"}</Button>
  </>}>
    <form id="power-editor" onSubmit={save} className={styles.form}>
      <Text as="p" tone="muted">{target ? "Describe the experience you want to build toward. Leave undecided fields unknown." : `Describe the list at r${assessedRevision}. Leave uncertain fields unknown and explain your evidence.`}</Text>
      <div className={styles.fields}>
        {fields.map(field => <Field key={field.key} label={field.label} hint={field.options.find(o => o.value === profile[field.key])?.hint ?? voice("Unknown until you assess it.", "Unknown until you or an agent assesses it.")}>
          <Select value={profile[field.key] ?? ""} onChange={e => setProfile({ ...profile, [field.key]: e.target.value || null })}>
            <option value="">Unknown / undecided</option>
            {field.options.map(o => <option key={o.value} value={o.value}>{o.label}</option>)}
          </Select>
        </Field>)}
        <Field label="Mass land denial" hint="Destroying, bouncing or shutting off several lands or their mana. Review this from card text and the deck's plan.">
          <Select value={profile.mass_land_denial == null ? "" : profile.mass_land_denial ? "yes" : "no"} onChange={e => setProfile({ ...profile, mass_land_denial: e.target.value === "" ? null : e.target.value === "yes" })}>
            <option value="">Unknown / undecided</option><option value="no">None</option><option value="yes">{target ? "Allowed" : "Present"}</option>
          </Select>
        </Field>
        <Field label="Usual win range (turns)" hint={target ? "An acceptable range when the deck normally threatens a win." : "Normal win attempts under representative conditions, rather than one perfect draw."}>
          <Row gap={3}>
            <TextField aria-label="First usual win turn" type="number" min={1} max={99} required={!!last} value={first} placeholder="From" onChange={e => setFirst(e.target.value)} />
            <Text>–</Text>
            <TextField aria-label="Last usual win turn" type="number" min={first || 1} max={99} required={!!first} value={last} placeholder="To" onChange={e => setLast(e.target.value)} />
          </Row>
        </Field>
        <Field label="Occasional early win (turn)" hint="Rare early wins stay separate. If this becomes frequent, include it in the usual range.">
          <TextField type="number" min={1} max={first || 99} value={early} placeholder="Not recorded" onChange={e => setEarly(e.target.value)} />
        </Field>
        {target && <Field label="Maximum Game Changers" hint="An intended limit. The app counts Game Changers in the current list from Scryfall.">
          <TextField type="number" min={0} max={100} value={limit} placeholder="No limit set" onChange={e => setLimit(e.target.value)} />
        </Field>}
        {!target && <>
          <Field label="Evidence"><Select value={evidence} onChange={e => setEvidence(e.target.value as typeof evidence)}>
            <option value="list">Reading the list</option><option value="goldfish">Goldfishing</option><option value="games">Actual games</option>
          </Select></Field>
          <Field label="Confidence" hint="Supported means there is evidence for these judgments, not that the bracket is certified."><Select value={confidence} onChange={e => setConfidence(e.target.value as typeof confidence)}>
            <option value="tentative">Tentative</option><option value="supported">Supported by evidence</option>
          </Select></Field>
        </>}
      </div>
      <Field label={target ? "Preferences and reasoning" : "Assessment reasoning"} hint={target ? "What matters most to your pod and what tradeoffs you want." : "Explain the win range, key lines, disruption and any observations. Card references use [[Card Name]]."}>
        <TextArea value={notes} required={!target} maxLength={6000} rows={4} onChange={e => setNotes(e.target.value)} />
      </Field>
      {error && <ErrorNote>{error}</ErrorNote>}
      {!target && <Text size="sm" tone="faint" as="p">Card edits will flag it for review.</Text>}
    </form>
  </Dialog>;
}
