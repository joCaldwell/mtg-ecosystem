# Design system

A dark, dense tool for long sessions of reading card text and making
decisions. The live reference is the style guide at `#/design`; this file is
the rules behind it.

## Principles

1. **The deck list is home.** Everything else comes to it: a side panel, a
   banner, a hover card. Nothing covers the list except a dialog you asked for.
2. **Colour means something or it is grey.** See the table below. A new
   feature does not get a new colour.
3. **Text first.** Cards are names, costs and oracle text. The picture appears
   on demand: in the hover card and the card panel.
4. **Dense, not cramped.** 13px body, 24px rows, one line per card. Space goes
   between groups, not between rows.
5. **Every agent write is visibly an agent's.** Violet marks it wherever it
   shows up, with the agent's name beside it.
6. **Reasons are cheap.** Any ruling can take a reason in one optional field,
   committed with Enter.
7. **Numbers are monospaced and come from the server.**

## What each colour means

| Token | Meaning | Examples |
| --- | --- | --- |
| `--agent` violet | An agent wrote or proposed it | proposal banner, memory chips, agent names, "waiting on you" |
| `--add` green | Coming in, passing, accepted | proposed add, a check that passes, Accept |
| `--cut` red | Going out, failing, rejected | proposed cut, a rule violation, Reject |
| `--warn` amber | Off target, missing, provisional | slot under target, incomplete deck |
| `--sel` blue | The owner's selection and focus | selected rows, search and tag matches, focus ring, focused field |
| greys | Everything else | structure, text, neutral chips |

Mana colours (`--mana-*`) are used only inside mana symbols and the pip chart.

Deck lifecycle labels use green for Paper, blue for Theorycrafted, and amber
for Legacy through the `--lifecycle-*` aliases. Sidebar labels and All decks
pills share these colors; sidebar labels appear only for Theorycrafted and
Legacy decks.

## Tokens

All in `src/design/tokens.css`: surfaces, text, the five meanings (each with
`-bg` and `-line`), mana, type sizes, a spacing scale (`--s-1` … `--s-8`),
radii, control heights, layout widths, elevation, stacking and motion.

Component CSS uses tokens only. If you reach for a literal, add a token.

## Type

- Body `--text-md` (13px). Secondary `--text-sm` (12px). Labels and meta
  `--text-xs` (11px).
- Page titles `--text-xl`, weight 650. Card and section titles `--text-lg`.
- Eyebrows (small section labels): `--text-xs`, uppercase, letter-spaced,
  `--faint`.
- Counts, revisions, mana values, times: `--font-mono` with tabular numerals.

## Writing in the UI

- Sentence case everywhere. No exclamation marks.
- Say what happens: "Accept the edit", not "OK".
- When something is only the owner's to see or do, say so in the place it
  applies ("Only you can set this, and agents never see it").
- Refer to revisions as `r95`, memories and proposals by their number.
- Empty states say what would put something there.

## Components

`ui/` holds the generic primitives and `mtg/` the card-specific ones. The
inventory, with every variant and state, is the style guide page at
`#/design`; keep it complete, because it is how a new page is made to match.

<!-- inventory:start -->
Everything below is exported from `ui/index.ts`. None of it knows about Magic
or about this app.

| Component | For | Variants |
| --- | --- | --- |
| `Button` | Any action | `default` `ghost` `primary` `accept` `reject` `agent`; `sm` `md`; `kbd` hint |
| `IconButton` | An action with only an icon; `label` is required | `sm` `md` |
| `Icon` | The 15 line icons (inbox, decks, memories, activity, profile, settings, back, close, check, plus, chevronDown, undo, redo, search, more) | `size` |
| `Kbd` | A key cap | |
| `Chip` | A small tag or filter; a button when given `onClick` | tone `neutral` `agent` `warn` `add` `cut`; `dashed` for unconfirmed |
| `Badge` | A count pill | `neutral` `agent` `warn` |
| `Dot` | A 6px status mark | `neutral` `agent` `warn` `add` `cut`; `pulse` |
| `Tabs` | Switching panels, underline style, arrow keys | per-tab `badge`, `tone: "agent"` |
| `Segmented` | Choosing one of a few views | |
| `Menu` | A text or icon button that opens a list of actions | trigger `variant` and `size`; `icon`; `align`; item `tone: "cut"`, `disabled` |
| `ContextMenu` | Pointer-positioned actions for a right-click or keyboard menu request | disabled items, separators, arrow navigation, focus restoration |
| `TextField` | One-line input | `fieldSize` `md` `lg`; `invalid` |
| `TextArea` | Multi-line input | `autosize` |
| `Select`, `Checkbox` | Native controls, restyled | |
| `MultiSelect` | Searchable checkbox dropdown with removable selected chips | controlled `value`, async `onChange`, empty state, `compact` trigger summary, dropdown `footer` |
| `Field` | Label, control and hint, wired together | |
| `Meter` | A bar toward a limit | `neutral` `agent` `warn` `add`; clamps to 0-100% |
| `Box` | A bordered container | tone `neutral` `agent` `warn`; `dashed`; padding `none` `sm` `md` |
| `Eyebrow`, `Section` | The small uppercase label, and a titled group with an `aside` | |
| `Stack`, `Row`, `Spacer` | Layout on the spacing scale `--s-1` to `--s-8` | `gap` 1-8; Row `align`, `justify`, `wrap` |
| `Text` | Text with a tone, size and weight from the tokens | tone: 8; size `xs` to `xl`; `mono`; weight 3 |
| `Banner` | A full-width strip under a header | `agent` `warn` `error` `neutral`; `actions` |
| `ErrorNote`, `EmptyState`, `Loading` | An inline error, "nothing here yet", and a loading line | |
| `Dialog` | A modal: focus trap, Esc, backdrop click, focus restore, `data-autofocus` | width `md` `lg`; `footer` |
| `ConfirmDialog` | A question with Cancel and a confirm button | tone `default` `danger` |
| `Popover`, `placeNear` | A panel anchored to an element, kept inside the viewport | placement `below` `right` `above` `left`; `align` |
| `ToastProvider`, `useToast` | One toast at a time, bottom centre, with an optional action | `duration`; `action` with `kbd` |
| `DiffView` | Added and deleted lines | line kind `same` `add` `del` |
| `Table`, `ClickableRow` | Plain tables with small uppercase headers; rows that open something | |
| `CounterControl` | Visible increment/decrement controls and a numeric readout | labeled, disabled |
| `KeyValue` | Two-column label and value list | |
| `ResizeHandle` | A draggable edge inside the element it resizes; arrow keys, Home, End, double-click to reset | `edge` `left` `right`; caller owns `value`, `min`, `max` |
| `Page`, `PageHeader`, `SplitPane` | Screen frame with a sticky header and scrolling body; list and detail columns | Page width `md` `lg` `full` |
| `useHotkeys` | Declare a screen's shortcuts (`"j"`, `"shift+a"`, `"mod+k"`); ignored while typing except Escape | `enabled`, `allowInInputs` |
| `useLocalStorage` | State kept in localStorage | |
| `tokenPx` | A size token as a number, where script has to do arithmetic with one | |
<!-- inventory:end -->


## Manual table extensions

The playtester deliberately uses larger rules text (`--text-rules`, 15px,
`--leading-rules`, 1.6) without changing the dense deck-list typography.
`mtg/table` contains text cards, zone containers, player summaries and card
inspection with host-supplied contextual tools. Full, compact, hidden, selected,
tapped, targeted and disabled observations are shown at `#/design`.

The table surface (`--table-bg`) is a step darker than the app's panels so
cards lift off it. Battlefield cards end in one line: fact chips (token,
counters, attachments) and a power/toughness box on the left, state and the tap
control on the right. A manual power/toughness box is dashed and says
"Manual". Tapped cards are dashed and quieter, never rotated; untapped cards
carry no label.

The battlefield uses three rows separated by simple rules. It has no visible
Battlefield heading, row headings or row counts. Hand and off-table zone counts
remain visible. The hand, battlefield, library drawer and inspection panel scroll
independently; fonts never shrink as a table grows. Selection has a blue border,
tapping has an explicit state/action and a dashed edge, and temporary inspection
never changes the pinned selection or an unfinished observation's target.

A hidden `CardObservation` has no card identity. Future game hosts must supply
only permitted observations. Deck memory and tag tools belong to the playtest
feature, never the shared card or zone presentation.
