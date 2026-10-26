import { useState } from "react";
import { Button, Dialog, Disclosure, ErrorNote, Explainer, MultiSelect, Row, Section, Stack, Text } from "../../ui/index.ts";
import styles from "./OnboardingPatterns.module.css";

/** Phase 4b inventory: live states, with the same primitives as the app. */
export function OnboardingPatterns() {
  const [advanced, setAdvanced] = useState(false);
  const [dialog, setDialog] = useState<"drawer" | "sheet" | null>(null);
  const [values, setValues] = useState<string[]>(["one"]);
  return <Section title="First-run and tablet patterns">
    <Stack gap={6}>
      <Text as="p" tone="muted">Tab reaches every trigger. Enter or Space opens it; Escape closes popovers and dialogs. The drawer and sheet trap focus and return it to their trigger. Hover and keyboard focus use the selection ring.</Text>
      <Disclosure open={advanced} onChange={setAdvanced}><Text>Advanced controls are visible. The app saves this preference per account.</Text></Disclosure>
      <Disclosure label="Advanced · open example" open onChange={() => {}}><Text>Revision, secondary purposes, memory budgets and review status.</Text></Disclosure>
      <Disclosure label="Advanced · saving" open={false} disabled onChange={() => {}}><Text>Disabled while a write is pending.</Text></Disclosure>
      <Stack gap={2}>
        <Disclosure label="Advanced · save failed" open={false} onChange={() => {}}><Text>Unchanged until a save succeeds.</Text></Disclosure>
        <ErrorNote>Couldn’t save the Advanced setting. Check your connection and try again.</ErrorNote>
      </Stack>
      <Row gap={5} wrap>
        <Text size="xs" tone="muted">Inline, beside deck metadata</Text>
        <Disclosure inline title="Show or hide revisions, secondary slots, budgets and review flags. Applies across your account." open={advanced} onChange={setAdvanced}><Text mono tone="faint">r95</Text></Disclosure>
      </Row>
      <Explainer title="Deck brief" summary="A brief records the plan behind the list.">Write what the deck is trying to do and what matters to you. Close this explanation to return to the same place. The default, hover, focus and expanded states use existing text and selection tokens.</Explainer>
      {/* Open from the start, with room below so the panel covers nothing else. */}
      <div className={styles.openExplainer}>
        <Explainer defaultOpen title="Slots and tags" summary="A slot says what a card is for; a tag tracks its sourcing or status.">Slots can have targets, counted on the group headings. Tags are your own words, such as owned or ordered. This example starts open.</Explainer>
      </div>
      <Stack gap={3}><Button onClick={() => setDialog("drawer")} aria-haspopup="dialog">Open navigation drawer</Button><Button onClick={() => setDialog("sheet")} aria-haspopup="dialog">Open details sheet</Button></Stack>
      <MultiSelect label="Example purposes" options={[{value:"one",label:"Primary support"},{value:"two",label:"Additional support"}]} value={values} onChange={async next => { await new Promise(resolve => setTimeout(resolve, 600)); setValues(next); }} />
      <MultiSelect label="Empty example" options={[]} value={[]} onChange={() => {}} />
      <h3>Archived title · plain text</h3>
      <Text as="p" tone="faint">Faint text stays readable on all app surfaces. Reduced motion turns off transitions and the pulse animation.</Text>
    </Stack>
    {dialog && <Dialog title={dialog === "drawer" ? "Navigation" : "Deck details"} placement={dialog} onClose={() => setDialog(null)}>
      <Disclosure label="Advanced in a dialog" open={advanced} onChange={setAdvanced}><Button>Hidden control excluded from focus trap</Button></Disclosure>
      <Stack gap={7}><Text>The {dialog} uses the same focus trap, Escape, backdrop and close control as Dialog.</Text><Button onClick={() => setDialog(null)}>Close {dialog}</Button></Stack>
    </Dialog>}
  </Section>;
}
