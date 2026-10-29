import { ArtExamples } from "./ArtExamples.tsx";
import { OnboardingPatterns } from "./OnboardingPatterns.tsx";
import { useState, type ReactNode } from "react";
import {
  Badge,
  Banner,
  Box,
  Button,
  ButtonLink,
  Checkbox,
  Chip,
  ClickableRow,
  ConfirmDialog,
  Dialog,
  DiffView,
  Dot,
  EmptyState,
  ErrorNote,
  Explainer,
  Eyebrow,
  Field,
  Icon,
  IconButton,
  Kbd,
  KeyValue,
  Loading,
  Menu,
  Meter,
  MultiSelect,
  Page,
  PageHeader,
  Pill,
  Popover,
  ResizeHandle,
  Row,
  Section,
  Segmented,
  Select,
  Spacer,
  SplitPane,
  Stack,
  Table,
  Tabs,
  Text,
  TextArea,
  TextField,
  useHotkeys,
  useLocalStorage,
  useToast,
  type Gap,
  type IconName,
} from "../../ui/index.ts";
import { TableExamples } from "./TableExamples.tsx";
import { MtgExamples } from "./MtgExamples.tsx";
import { ContextMenuExample } from "./ContextMenuExample.tsx";
import styles from "./StyleguidePage.module.css";

// ---------------------------------------------------------------- data

interface Token {
  name: string;
  note: string;
}

const SURFACES: Token[] = [
  { name: "--bg", note: "True-black canvas and sidebar" },
  { name: "--surface", note: "Near-black cards, panels and inputs" },
  { name: "--raised", note: "Brighter selected surfaces and neutral chips" },
  { name: "--hover", note: "4% white over the current surface" },
  { name: "--overlay", note: "Opaque near-black dialogs, menus and toasts" },
  { name: "--line", note: "8% white hairline dividers" },
  { name: "--line-strong", note: "11% white control borders" },
  { name: "--line-hover", note: "18% white hovered borders" },
  { name: "--action", note: "Blue primary actions, black ink" },
];

const TEXTS: Token[] = [
  { name: "--text", note: "Primary text" },
  { name: "--text-soft", note: "Prose and labels" },
  { name: "--muted", note: "Secondary text, resting icons" },
  { name: "--faint", note: "Eyebrows, placeholders, meta" },
];

const MEANINGS: Array<{ name: string; means: string; use: string }> = [
  { name: "agent", means: "An agent wrote or proposed it", use: "proposal banner, memory chips, agent names, waiting on you" },
  { name: "add", means: "Coming in, passing, accepted", use: "proposed add, a passing check, Accept" },
  { name: "cut", means: "Going out, failing, rejected", use: "proposed cut, a rule violation, Reject" },
  { name: "warn", means: "Off target, missing, provisional", use: "slot under target, incomplete deck" },
  { name: "sel", means: "The owner's selection and focus", use: "selected rows, focus ring, focused field" },
];

const MANA: Token[] = [
  { name: "--mana-w", note: "White" },
  { name: "--mana-u", note: "Blue" },
  { name: "--mana-b", note: "Black (with --mana-b-ring)" },
  { name: "--mana-r", note: "Red" },
  { name: "--mana-g", note: "Green" },
  { name: "--mana-c", note: "Colourless" },
  { name: "--mana-generic", note: "Generic cost" },
];

const SIZES = [
  { token: "--text-2xs", px: "10px", sample: "Key caps and the smallest tags" },
  { token: "--text-xs", px: "11px", sample: "Labels, eyebrows, meta" },
  { token: "--text-sm", px: "12px", sample: "Secondary text, small buttons, table cells" },
  { token: "--text-md", px: "13px", sample: "Body text: card names, oracle text, fields" },
  { token: "--text-lg", px: "15px", sample: "Card and section titles" },
  { token: "--text-xl", px: "17px", sample: "Page titles" },
] as const;

const SPACE: Gap[] = [1, 2, 3, 4, 5, 6, 7, 8];
const SPACE_PX = ["2px", "4px", "6px", "8px", "12px", "16px", "24px", "32px"];

const RADII = ["--r-sm", "--r-md", "--r-lg", "--r-xl", "--r-pill"] as const;

const ICONS: IconName[] = [
  "inbox", "decks", "memories", "activity", "profile", "settings", "back",
  "close", "check", "plus", "chevronDown", "undo", "redo", "search", "more", "globe", "link",
];

type Tone = "neutral" | "agent" | "warn" | "add" | "cut";
const TONES: Tone[] = ["neutral", "agent", "warn", "add", "cut"];

// ---------------------------------------------------------------- helpers

function Swatch({ token, note }: { token: string; note: string }) {
  return (
    <div className={styles.swatch}>
      <div className={styles.chipface} style={{ background: `var(${token})` }} />
      <div>
        <div className={styles.swatchName}>{token}</div>
        <div className={styles.swatchNote}>{note}</div>
      </div>
    </div>
  );
}

function Demo({ children, note }: { children: ReactNode; note?: ReactNode }) {
  return (
    <div className={styles.demo}>
      {children}
      {note ? <div className={styles.demoNote}>{note}</div> : null}
    </div>
  );
}

function Label({ children }: { children: ReactNode }) {
  return (
    <Text size="xs" tone="faint" mono>
      {children}
    </Text>
  );
}

// ---------------------------------------------------------------- sections

function Colours() {
  return (
    <Stack gap={7}>
      <Section title="Colour: surfaces">
        <div className={styles.swatches}>
          {SURFACES.map((t) => <Swatch key={t.name} token={t.name} note={t.note} />)}
        </div>
      </Section>
      <Section title="Colour: text" aside="AA minima including nested tints: 11.51 / 8.47 / 4.97 / 4.57:1">
        <div className={styles.swatches}>
          {TEXTS.map((t) => <Swatch key={t.name} token={t.name} note={t.note} />)}
        </div>
      </Section>
      <Section title="Colour: what each one means" aside="each has a 6% -bg tint and a 32% -line border">
        <Stack gap={4}>
          {MEANINGS.map((m) => (
            <Row key={m.name} gap={5} align="center">
              <div className={styles.triple}>
                {["", "-bg", "-line"].map((suffix) => (
                  <div key={suffix} className={styles.chipface} style={{ background: `var(--${m.name}${suffix})` }} />
                ))}
              </div>
              <Stack gap={1}>
                <Text weight="medium">
                  <span style={{ color: `var(--${m.name})` }}>--{m.name}</span>{" "}
                  <Text tone="muted">{m.means}</Text>
                </Text>
                <Text size="xs" tone="faint">{m.use}</Text>
              </Stack>
            </Row>
          ))}
          <Text size="sm" tone="muted">Anything else is grey. A new feature does not get a new colour.</Text>
        </Stack>
      </Section>
      <Section title="Colour: mana" aside="only inside mana symbols and the pip chart">
        <div className={styles.swatches}>
          {MANA.map((t) => <Swatch key={t.name} token={t.name} note={t.note} />)}
        </div>
      </Section>
    </Stack>
  );
}

function TypeScale() {
  return (
    <Stack gap={7}>
      <Section title="Type scale">
        <div>
          {SIZES.map((s) => (
            <div key={s.token} className={styles.typeRow}>
              <Label>{s.token} {s.px}</Label>
              <span style={{ fontSize: `var(${s.token})` }}>{s.sample}</span>
            </div>
          ))}
          <div className={styles.typeRow}>
            <Label>--font-mono</Label>
            <Text mono>r95  r96  1,204  100/100  12:04</Text>
          </div>
          <div className={styles.typeRow}>
            <Label>Text weights</Label>
            <Row gap={6}>
              <Text weight="normal">normal</Text>
              <Text weight="medium">medium</Text>
              <Text weight="bold">bold</Text>
            </Row>
          </div>
          <div className={styles.typeRow}>
            <Label>Text tones</Label>
            <Row gap={6} wrap>
              {(["default", "soft", "muted", "faint", "agent", "add", "cut", "warn"] as const).map((t) => (
                <Text key={t} tone={t}>{t}</Text>
              ))}
            </Row>
          </div>
        </div>
      </Section>
      <Section title="Spacing scale">
        <Stack gap={3}>
          {SPACE.map((n) => (
            <div key={n} className={styles.spaceRow}>
              <Label>--s-{n} {SPACE_PX[n - 1]}</Label>
              <div className={styles.bar} style={{ width: `var(--s-${n})` }} />
            </div>
          ))}
        </Stack>
      </Section>
      <Section title="Radii">
        <Row gap={5} wrap>
          {RADII.map((r) => (
            <div key={r} className={styles.radius} style={{ borderRadius: `var(${r})` }}>{r.slice(2)}</div>
          ))}
        </Row>
      </Section>
    </Stack>
  );
}

function Buttons() {
  const [pillOn, setPillOn] = useState(false);
  const variants = ["default", "ghost", "primary", "accept", "reject", "agent"] as const;
  return (
    <Stack gap={7}>
      <Section title="Button">
        <Demo note="Rows: md, sm, touch (one --tap tall, for phones), with a key hint, disabled. Disabled is one neutral look for every variant.">
          {[{}, { size: "sm" as const }, { size: "touch" as const }, { kbd: "A" }, { disabled: true }].map((extra, i) => (
            <Row key={i} gap={4} wrap>
              {variants.map((v) => (
                <Button key={v} variant={v} {...extra}>{v}</Button>
              ))}
            </Row>
          ))}
        </Demo>
        <Demo note="ButtonLink: a link that goes somewhere, drawn as a button (default, ghost, primary).">
          <Row gap={4} wrap>
            <ButtonLink href="#button-link">Sign in to fork or compare</ButtonLink>
            <ButtonLink href="#button-link" variant="ghost">ghost</ButtonLink>
            <ButtonLink href="#button-link" variant="primary">primary</ButtonLink>
          </Row>
        </Demo>
      </Section>
      <Section title="Pill filters">
        <Demo note="Selectable filters show their pressed state; click either example.">
          <Row><Pill on={pillOn} onClick={() => setPillOn(true)}>Selected</Pill><Pill on={!pillOn} onClick={() => setPillOn(false)}>Other filter</Pill></Row>
        </Demo>
      </Section>
      <Section title="IconButton">
        <Demo>
          <Row gap={3}>
            {(["close", "more", "undo", "search", "plus", "back"] as IconName[]).map((n) => (
              <IconButton key={n} label={n}><Icon name={n} /></IconButton>
            ))}
            <Spacer />
            <Label>sm</Label>
            {(["close", "more"] as IconName[]).map((n) => (
              <IconButton key={n} label={n} size="sm"><Icon name={n} /></IconButton>
            ))}
            <Label>touch</Label>
            <IconButton label="close" size="touch"><Icon name="close" /></IconButton>
          </Row>
        </Demo>
      </Section>
      <Section title="Icon" aside="16x16, stroke, follows text colour">
        <Demo>
          <Row gap={6} wrap>
            {ICONS.map((n) => (
              <Stack key={n} gap={3} className={styles.iconCell}>
                <Icon name={n} size={20} />
                <Label>{n}</Label>
              </Stack>
            ))}
          </Row>
        </Demo>
      </Section>
      <Section title="Kbd">
        <Demo>
          <Row gap={4}>
            <Kbd>J</Kbd><Kbd>Shift</Kbd><Kbd>Enter</Kbd><Kbd>Esc</Kbd>
            <Text tone="muted" size="sm">Next card</Text><Kbd>J</Kbd>
          </Row>
        </Demo>
      </Section>
    </Stack>
  );
}

function Labels() {
  return (
    <Stack gap={7}>
      <Section title="Chip">
        <Demo note="Dashed is for things not yet confirmed. With onClick a chip is a button.">
          <Row gap={4} wrap>
            {TONES.map((t) => <Chip key={t} tone={t}>{t}</Chip>)}
          </Row>
          <Row gap={4} wrap>
            {TONES.map((t) => <Chip key={t} tone={t} dashed>{t} dashed</Chip>)}
          </Row>
          <Row gap={4} wrap>
            <Chip onClick={() => undefined}>clickable</Chip>
            <Chip tone="agent" onClick={() => undefined}>clickable agent</Chip>
          </Row>
        </Demo>
      </Section>
      <Section title="Badge">
        <Demo>
          <Row gap={4}>
            <Badge>3</Badge><Badge tone="agent">12</Badge><Badge tone="warn">99</Badge>
          </Row>
        </Demo>
      </Section>
      <Section title="Dot">
        <Demo>
          <Row gap={6}>
            {TONES.map((t) => <Row key={t} gap={3}><Dot tone={t} /><Label>{t}</Label></Row>)}
            {TONES.map((t) => <Row key={`p${t}`} gap={3}><Dot tone={t} pulse /><Label>{t} pulse</Label></Row>)}
          </Row>
        </Demo>
      </Section>
      <Section title="Eyebrow and Section">
        <Demo>
          <Eyebrow>Eyebrow on its own</Eyebrow>
          <Section title="Section" aside={<Chip tone="agent">aside</Chip>}>
            <Text tone="soft">A Section is an Eyebrow, an optional aside, and its content in a small stack.</Text>
          </Section>
        </Demo>
      </Section>
    </Stack>
  );
}

function Navigation() {
  const [tab, setTab] = useState("list");
  const [view, setView] = useState<"grid" | "list" | "text">("list");
  return (
    <Stack gap={7}>
      <Section title="Sidebar">
        <Text tone="muted">All decks comes first. Profile holds Profile and Activity tabs. At 1100 px and wider, the sidebar’s collapse control saves your account’s choice; icons, inbox counts and named deck thumbnails stay visible in the rail. Tablet navigation keeps its labels.</Text>
      </Section>
      <Section title="Tabs" aside="arrow keys move between tabs">
        <Demo>
          <Tabs
            tabs={[
              { id: "list", label: "Deck" },
              { id: "review", label: "Review", badge: 4, tone: "agent" },
              { id: "notes", label: "Notes", badge: 2 },
              { id: "log", label: "Log" },
            ]}
            value={tab}
            onChange={setTab}
            aria-label="Example tabs"
          />
          <Text size="sm" tone="muted">Selected: {tab}</Text>
        </Demo>
      </Section>
      <Section title="Segmented">
        <Demo>
          <Row gap={5}>
            <Segmented
              label="View"
              value={view}
              onChange={setView}
              options={[
                { value: "grid", label: "Grid", title: "Card images" },
                { value: "list", label: "List", title: "One line per card" },
                { value: "text", label: "Text" },
              ]}
            />
            <Text size="sm" tone="muted">{view}</Text>
          </Row>
        </Demo>
      </Section>
      <ContextMenuExample />
      <Section title="Menu" aside="up and down move, Enter selects, Esc closes">
        <Demo>
          <Row gap={4}>
            <Menu
              label="Actions"
              items={[
                { id: "a", label: "Rename", onSelect: () => undefined },
                { id: "b", label: "Duplicate", onSelect: () => undefined },
                { id: "c", label: "Export (not available)", onSelect: () => undefined, disabled: true },
                { id: "d", label: "Delete deck", tone: "cut", onSelect: () => undefined },
              ]}
            />
            <Menu
              label="Small, right-aligned"
              size="sm"
              variant="ghost"
              align="end"
              items={[{ id: "a", label: "One", onSelect: () => undefined }, { id: "b", label: "Two", onSelect: () => undefined }]}
            />
            <LifecyclePillDemo />
            <Menu
              label="More actions"
              icon="more"
              size="sm"
              align="end"
              items={[
                { id: "a", label: "Rename", onSelect: () => undefined },
                { id: "b", label: "Delete", tone: "cut", onSelect: () => undefined },
              ]}
            />
          </Row>
        </Demo>
      </Section>
    </Stack>
  );
}

/** A pill trigger with a single-choice list: the current state, changed from
 *  its menu. A last item, set apart, opens an explanation beside the pill. */
function LifecyclePillDemo() {
  const [value, setValue] = useState("Paper");
  const [pill, setPill] = useState<HTMLButtonElement | null>(null);
  const [explaining, setExplaining] = useState(false);
  return (
    <>
      <Menu
        pill
        label={value}
        ariaLabel={`Lifecycle: ${value}`}
        triggerRef={setPill}
        items={[
          ...["Paper", "Theorycrafted", "Archived"].map((v) => ({ id: v, label: v, checked: v === value, onSelect: () => setValue(v) })),
          { id: "help", label: "Who can edit?", separated: true, onSelect: () => setExplaining(true) },
        ]}
      />
      <Explainer title="Deck lifecycle" control={{ anchor: pill, open: explaining, onClose: () => setExplaining(false) }}>
        An explanation opened from a menu item sits beside the menu's trigger and returns focus to it.
      </Explainer>
    </>
  );
}

function Forms() {
  const [text, setText] = useState("");
  const [area, setArea] = useState("This one grows as you type.\nTry adding lines.");
  const [checked, setChecked] = useState(true);
  const [sel, setSel] = useState("b");
  const [requiredFormat, setRequiredFormat] = useState("");
  const [multi, setMulti] = useState<string[]>([]);
  return (
    <Stack gap={7}>
      <Section title="TextField">
        <Demo>
          <Stack gap={4} className={styles.narrow}>
            <TextField placeholder="md (default)" value={text} onChange={(e) => setText(e.target.value)} />
            <TextField fieldSize="lg" placeholder="lg: for the add bar" />
            <TextField aria-label="Invalid example" invalid defaultValue="invalid" />
            <TextField aria-label="Disabled example" disabled defaultValue="disabled" />
          </Stack>
        </Demo>
      </Section>
      <Section title="TextArea">
        <Demo>
          <Stack gap={4} className={styles.narrow}>
            <TextArea placeholder="Fixed rows" rows={2} />
            <TextArea aria-label="Autosizing example" autosize value={area} onChange={(e) => setArea(e.target.value)} />
          </Stack>
        </Demo>
      </Section>
      <Section title="Select and Checkbox">
        <Demo>
          <Row gap={6} wrap>
            <Select aria-label="Select example" value={sel} onChange={(e) => setSel(e.target.value)}>
              <option value="a">Commander</option>
              <option value="b">Brawl</option>
              <option value="c">Standard</option>
            </Select>
            <Select aria-label="Disabled select example" disabled defaultValue="a"><option value="a">Disabled</option></Select>
            <Select aria-label="Required format" aria-invalid={!requiredFormat || undefined} required value={requiredFormat} onChange={e => setRequiredFormat(e.target.value)}>
              <option value="" disabled>Choose a format…</option>
              <option value="commander">Commander</option>
              <option value="brawl">Brawl</option>
            </Select>
            <Checkbox checked={checked} onChange={setChecked} label="Checked" />
            <Checkbox checked={false} onChange={() => undefined} label="Unchecked" />
            <Checkbox checked disabled onChange={() => undefined} label="Disabled" />
          </Row>
        </Demo>
      </Section>
      <Section title="MultiSelect" aside="search, check several choices, remove selected chips">
        <Demo>
          <Stack gap={5} className={styles.narrow}>
            <MultiSelect label="Example tags" placeholder="Select tags…" options={[{ value: "a", label: "Need to get" }, { value: "b", label: "Engine piece" }, { value: "c", label: "A longer label that wraps when space is limited" }]} value={multi} onChange={setMulti} />
            <MultiSelect label="Empty example" options={[]} value={[]} onChange={() => undefined} />
            <MultiSelect compact label="Compact example" placeholder="Select tags…" options={[{ value: "a", label: "Need to get" }, { value: "b", label: "Engine piece" }, { value: "c", label: "A longer label that wraps when space is limited" }]} value={multi} onChange={setMulti} footer={<Button size="sm" variant="ghost" onClick={() => setMulti([])}>Clear selection</Button>} />
            <MultiSelect compact label="Custom summary example" summary="Not Have" options={[{ value: "have", label: "Have" }]} value={["have"]} onChange={() => undefined} />
          </Stack>
        </Demo>
      </Section>
      <Section title="Field" aside="label above, hint below, wired to the control">
        <Demo>
          <Stack gap={5} className={styles.narrow}>
            <Field label="Deck name" hint="Shown in the sidebar."><TextField defaultValue="Jo's upkeep" /></Field>
            <Field label="Format"><Select defaultValue="b"><option value="b">Commander</option></Select></Field>
            <Field label="Brief" hint="Only you can set this."><TextArea rows={2} /></Field>
          </Stack>
        </Demo>
      </Section>
    </Stack>
  );
}

function Feedback() {
  return (
    <Stack gap={7}>
      <Section title="Meter">
        <Demo>
          <Stack gap={4} className={styles.narrow}>
            {(["neutral", "agent", "warn", "add"] as const).map((t, i) => (
              <Row key={t} gap={5}>
                <span className={styles.meterLabel}><Label>{t}</Label></span>
                <div className={styles.grow}><Meter tone={t} value={(i + 1) * 20} max={100} aria-label={t} /></div>
              </Row>
            ))}
            <Row gap={5}><span className={styles.meterLabel}><Label>over</Label></span><div className={styles.grow}><Meter aria-label="Over budget example" tone="warn" value={140} max={100} /></div></Row>
            <Row gap={5}><span className={styles.meterLabel}><Label>empty</Label></span><div className={styles.grow}><Meter aria-label="Empty meter example" value={0} max={100} /></div></Row>
          </Stack>
        </Demo>
      </Section>
      <Section title="Box">
        <Demo>
          <Row gap={5} align="start" wrap>
            <Box className={styles.boxDemo}>Neutral</Box>
            <Box tone="agent" className={styles.boxDemo}>Agent</Box>
            <Box tone="warn" className={styles.boxDemo}>Warn</Box>
            <Box dashed className={styles.boxDemo}>Dashed</Box>
            <Box tone="agent" dashed padding="sm" className={styles.boxDemo}>Agent, dashed, sm</Box>
            <Box padding="none" className={styles.boxDemo}>No padding</Box>
          </Row>
        </Demo>
      </Section>
      <Section title="Banner">
        <div className={styles.bannerFrame}>
          <Banner tone="agent" actions={<><Button size="sm" variant="reject">Reject all</Button><Button size="sm" variant="accept" kbd="A">Accept all</Button></>}>
            <Text weight="medium">Claude Code</Text> proposed 6 changes to this deck.
          </Banner>
          <Banner tone="warn" actions={<Button size="sm">Fork</Button>}>This Archived deck is read-only.</Banner>
          <Banner tone="error">Could not reach the server.</Banner>
          <Banner tone="neutral">Nothing is waiting on you.</Banner>
        </div>
      </Section>
      <Section title="ErrorNote, EmptyState, Loading">
        <Demo>
          <ErrorNote>Could not save: the deck was changed in another window.</ErrorNote>
          <EmptyState title="No memories yet">Ask an agent to remember something, or add one here.</EmptyState>
          <Loading />
        </Demo>
      </Section>
      <Section title="DiffView">
        <Stack gap={4}>
        <DiffView
          lines={[
            { kind: "same", text: "Aggro-leaning shell with a graveyard subtheme." },
            { kind: "del", text: "Keep the curve under 3 average." },
            { kind: "add", text: "Keep the curve under 2.8 average; two cards at 5 or more." },
            { kind: "same", text: "Never play wraths." },
          ]}
        />
        <DiffView
          foldSame
          lines={[
            { kind: "same", text: "Aggro-leaning shell with a graveyard subtheme." },
            { kind: "same", text: "Two cards at 5 or more." },
            { kind: "same", text: "Never play wraths." },
            { kind: "add", text: "No tutors that cost more than 2." },
          ]}
        />
        <Text size="sm" tone="muted">foldSame: only the changes; each run of unchanged lines is one row that opens in place.</Text>
        </Stack>
      </Section>
      <ToastDemo />
    </Stack>
  );
}

function ToastDemo() {
  const { toast, dismiss } = useToast();
  return (
    <Section title="Toast" aside="one at a time, bottom centre, replaced by the next">
      <Demo>
        <Row gap={4} wrap>
          <Button onClick={() => toast("Deck saved.")}>Plain toast</Button>
          <Button onClick={() => toast("Removed Sol Ring.", { action: { label: "Undo", kbd: "Z", onAction: () => toast("Restored.") } })}>
            With action
          </Button>
          <Button onClick={() => toast("Stays for 20 seconds.", { duration: 20000 })}>Long</Button>
          <Button variant="ghost" onClick={dismiss}>Dismiss</Button>
        </Row>
      </Demo>
    </Section>
  );
}

function Overlays() {
  const [dialog, setDialog] = useState<null | "md" | "lg" | "wide" | "sheet" | "confirm" | "danger">(null);
  const [pop, setPop] = useState<null | "below" | "right" | "above" | "left">(null);
  const [anchorEl, setAnchorEl] = useState<HTMLButtonElement | null>(null);
  const [note, setNote] = useState("");
  return (
    <Stack gap={7}>
      <Section title="Dialog" aside="Esc or a click outside closes; Tab stays inside; focus returns">
        <Demo>
          <Row gap={4} wrap>
            <Button onClick={() => setDialog("md")}>Dialog md</Button>
            <Button onClick={() => setDialog("lg")}>Dialog lg</Button>
            <Button onClick={() => setDialog("wide")}>Dialog wide</Button>
            <Button onClick={() => setDialog("sheet")}>Dialog sheet (touch)</Button>
            <Button onClick={() => setDialog("confirm")}>Confirm</Button>
            <Button variant="reject" onClick={() => setDialog("danger")}>Confirm, danger</Button>
          </Row>
        </Demo>
        {dialog === "md" || dialog === "lg" || dialog === "wide" || dialog === "sheet" ? (
          <Dialog
            title="Rename deck"
            width={dialog === "sheet" ? "md" : dialog}
            placement={dialog === "sheet" ? "sheet" : "center"}
            onClose={() => setDialog(null)}
            footer={<><Button variant="ghost" onClick={() => setDialog(null)}>Cancel</Button><Button variant="primary" onClick={() => setDialog(null)}>Save</Button></>}
          >
            <Field label="Name" hint="Autofocused through data-autofocus.">
              <TextField aria-label="Deck name" data-autofocus defaultValue="Jo's upkeep" />
            </Field>
          </Dialog>
        ) : null}
        {dialog === "confirm" || dialog === "danger" ? (
          <ConfirmDialog
            title={dialog === "danger" ? "Delete this deck?" : "Activate this deck?"}
            tone={dialog === "danger" ? "danger" : "default"}
            confirmLabel={dialog === "danger" ? "Delete deck" : "Activate"}
            onConfirm={() => setDialog(null)}
            onClose={() => setDialog(null)}
          >
            {dialog === "danger" ? "Its history and memories go with it. This cannot be undone." : "Agents will start treating it as the current list."}
          </ConfirmDialog>
        ) : null}
      </Section>
      <Section title="Popover and placeNear" aside="flips when there is no room, clamps to the viewport">
        <Demo>
          <Row gap={4} wrap>
            {(["below", "right", "above", "left"] as const).map((p) => (
              <Button
                key={p}
                onClick={(e) => { setAnchorEl(e.currentTarget); setPop(pop === p ? null : p); }}
              >
                {p}
              </Button>
            ))}
          </Row>
          <Popover anchor={anchorEl} open={pop !== null} placement={pop ?? "below"} onClose={() => setPop(null)}>
            <div className={styles.popBody}>
              <Text tone="soft">Placed {pop}.</Text>
            </div>
          </Popover>
        </Demo>
      </Section>
      <Section title="useHotkeys and useLocalStorage" aside="hooks, shown here because they are part of ui/">
        <HooksDemo note={note} setNote={setNote} />
      </Section>
    </Stack>
  );
}

function HooksDemo({ note, setNote }: { note: string; setNote: (v: string) => void }) {
  const [last, setLast] = useState("(press j, shift+a, mod+k or /)");
  const [count, setCount] = useLocalStorage("styleguide.count", (raw) => Number(raw ?? 0) || 0, String);
  useHotkeys({
    j: () => setLast("j"),
    "shift+a": () => setLast("shift+a"),
    "mod+k": (e) => { e.preventDefault(); setLast("mod+k"); },
    "/": (e) => { e.preventDefault(); setLast("/"); },
  });
  return (
    <Demo note="Hotkeys are ignored while typing in a field, except Escape.">
      <Row gap={5}>
        <Label>last key</Label><Text mono className={styles.hotkeyLog}>{last}</Text>
      </Row>
      <Row gap={5}>
        <Label>stored count</Label>
        <Button size="sm" onClick={() => setCount(count + 1)}>Add one</Button>
        <Text mono>{count}</Text>
        <Button size="sm" variant="ghost" onClick={() => setCount(0)}>Reset</Button>
      </Row>
      <TextField className={styles.narrow} placeholder="Typing here does not trigger j" value={note} onChange={(e) => setNote(e.target.value)} />
    </Demo>
  );
}

const RESIZE_DEMO = { initial: 280, min: 200, max: 480 };

function ResizeDemo() {
  const [width, setWidth] = useState(RESIZE_DEMO.initial);
  return (
    <Demo note="Drag the right edge, or focus it and use the arrow keys, Home and End. Double-click resets. The caller owns the width and its limits.">
      <div className={styles.resizable} style={{ width }}>
        <Text mono tone="muted">{width}px, between {RESIZE_DEMO.min} and {RESIZE_DEMO.max}</Text>
        <ResizeHandle
          label="Resize the example"
          edge="right"
          value={width}
          min={RESIZE_DEMO.min}
          max={RESIZE_DEMO.max}
          onChange={setWidth}
          onReset={() => setWidth(RESIZE_DEMO.initial)}
        />
      </div>
    </Demo>
  );
}

function Layout() {
  const [picked, setPicked] = useState("one");
  return (
    <Stack gap={7}>
      <Section title="Stack, Row, Spacer">
        <Demo>
          <Row gap={7} align="start">
            <Stack gap={2} className={styles.stackDemo}><span>gap 2</span><span>gap 2</span><span>gap 2</span></Stack>
            <Stack gap={6} className={styles.stackDemo}><span>gap 6</span><span>gap 6</span><span>gap 6</span></Stack>
          </Row>
          <Row gap={4} className={styles.rowDemo}><span>start</span><span>start</span></Row>
          <Row justify="between" className={styles.rowDemo}><span>between</span><span>between</span></Row>
          <Row justify="end" className={styles.rowDemo}><span>end</span><span>end</span></Row>
          <Row className={styles.rowDemo}><span>left</span><Spacer /><span>right (Spacer)</span></Row>
          <Row align="baseline" className={styles.rowDemo}><Text size="xl">baseline</Text><span>baseline</span></Row>
        </Demo>
      </Section>
      <Section title="Table and ClickableRow">
        <div className={styles.tableWrap}>
          <Table>
            <thead><tr><th>Rev</th><th>Who</th><th>Change</th><th>When</th></tr></thead>
            <tbody>
              {[["r95", "You", "Cut Mind Stone", "2 min ago"], ["r94", "Claude Code", "Added Arcane Signet", "12 min ago"], ["r93", "You", "Set the brief", "yesterday"]].map(([a, b, c, d]) => (
                <ClickableRow key={a} onClick={() => setPicked(a ?? "")}>
                  <td><Text mono tone="muted">{a}</Text></td>
                  <td>{b === "You" ? b : <Text tone="agent">{b}</Text>}</td>
                  <td>{c}</td>
                  <td><Text tone="faint">{d}</Text></td>
                </ClickableRow>
              ))}
            </tbody>
          </Table>
        </div>
        <Text size="xs" tone="faint">Clicked row: {picked}</Text>
      </Section>
      <Section title="KeyValue">
        <Demo>
          <KeyValue items={[["Format", "Commander"], ["Cards", <Text key="c" mono>99 / 100</Text>], ["Lifecycle", <Chip key="s">Theorycrafted</Chip>]]} />
        </Demo>
      </Section>
      <Section title="ResizeHandle">
        <ResizeDemo />
      </Section>
      <Section title="Page, PageHeader, SplitPane">
        <div className={styles.framed}>
          <Page
            width="full"
            header={<PageHeader title="Memories" actions={<><Button size="sm">Filter</Button><Button size="sm" variant="primary">New memory</Button></>} />}
          >
            <SplitPane
              list={<><Box padding="sm">First item</Box><Box padding="sm">Second item</Box></>}
              detail={<Text tone="muted">The detail column scrolls on its own.</Text>}
            />
          </Page>
        </div>
      </Section>
    </Stack>
  );
}

export function StyleguidePage() {
  return (
    <Page
      width="lg"
      header={<PageHeader title="Design" />}
    >
      <div className={styles.groups}>
        <OnboardingPatterns />
        <ArtExamples />
        <Colours />
        <TypeScale />
        <Buttons />
        <Labels />
        <Navigation />
        <Forms />
        <Feedback />
        <Overlays />
        <Layout />
        <Section title="Card components">
          <MtgExamples />
          <TableExamples />
        </Section>
      </div>
    </Page>
  );
}
