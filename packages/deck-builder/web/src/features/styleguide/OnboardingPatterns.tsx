import { useState } from "react";
import { Button, Dialog, Explainer, KeyValue, MultiSelect, Select, Section, Stack, Text } from "../../ui/index.ts";
import styles from "./OnboardingPatterns.module.css";

/** Phase 4b inventory: live states, with the same primitives as the app. */
export function OnboardingPatterns() {
  const [dialog, setDialog] = useState<"drawer" | "sheet" | null>(null);
  const [secondary, setSecondary] = useState<string[]>(["one"]);
  const [values, setValues] = useState<string[]>(["one"]);
  return <Section title="First-run and tablet patterns">
    <Stack gap={6}>
      <Text as="p" tone="muted">Tab reaches every trigger. Enter or Space opens it; Escape closes popovers and dialogs. The drawer and sheet trap focus and return it to their trigger. Hover and keyboard focus use the selection ring.</Text>
      <Section title="Contextual details">
        <KeyValue items={[
          ["History", <Text mono size="sm">r95</Text>],
          ["Review status", "Unreviewed"],
          ["Primary slot", <Select aria-label="Example primary slot"><option>Support</option></Select>],
          ["Secondary slots", <MultiSelect compact label="Example secondary slots" options={[{ value: "one", label: "Utility" }]} value={secondary} onChange={setSecondary} />],
          ["Updated", <Text size="sm" title="Recorded at r95">Today</Text>],
        ]} />
      </Section>
      <Explainer title="Proposals" label="How proposals work">Accept or reject proposed changes with an optional reason. Help appears only for unfamiliar concepts and names the question it answers.</Explainer>
      {/* Open from the start, with room below so the panel covers nothing else. */}
      <div className={styles.openExplainer}>
        <Explainer defaultOpen title="Slots and tags" label="How slot targets work">Slots can have targets, counted on the group headings. Tags are your own words, such as owned or ordered. This example starts open.</Explainer>
      </div>
      <Stack gap={3}><Button onClick={() => setDialog("drawer")} aria-haspopup="dialog">Open navigation drawer</Button><Button onClick={() => setDialog("sheet")} aria-haspopup="dialog">Open details sheet</Button></Stack>
      <MultiSelect label="Example purposes" options={[{value:"one",label:"Primary support"},{value:"two",label:"Additional support"}]} value={values} onChange={async next => { await new Promise(resolve => setTimeout(resolve, 600)); setValues(next); }} />
      <MultiSelect label="Empty example" options={[]} value={[]} onChange={() => {}} />
      <h3>Archived title · plain text</h3>
      <Text as="p" tone="faint">Faint text stays readable on all app surfaces. Reduced motion turns off transitions and the pulse animation.</Text>
    </Stack>
    {dialog && <Dialog title={dialog === "drawer" ? "Navigation" : "Deck details"} placement={dialog} onClose={() => setDialog(null)}>
      <Stack gap={7}><Text>The {dialog} uses the same focus trap, Escape, backdrop and close control as Dialog.</Text><Button onClick={() => setDialog(null)}>Close {dialog}</Button></Stack>
    </Dialog>}
  </Section>;
}
