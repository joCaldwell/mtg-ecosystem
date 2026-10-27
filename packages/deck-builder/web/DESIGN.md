# Design system

A dark, dense tool for long sessions of reading card text and making
decisions. The live reference is the style guide at `/design`; this file is
the rules behind it.

## Principles

1. **The deck list is home.** Everything else comes to it: a side panel, a
   banner, a hover card. Nothing covers the list except a dialog you asked for.
2. **Colour means something or it is neutral.** The canvas and sidebar are
   true black. Near-black surfaces lift slightly; transparent white hairlines
   and hover states separate them. No blue-tinted greys or milky fills. One
   blue action colour serves primary actions; the meanings below stay distinct.
   A new feature does not get a new colour.
3. **Text first in the editor.** Card rows remain names, costs and oracle
   text, with pictures on demand. Commander art may frame deck selection and
   deck headers; a quiet environment may frame sign-in and first run (D21).
4. **Dense, not cramped.** 13px body, 24px rows, one line per card. Space goes
   between groups, not between rows.
5. **Every agent write is visibly an agent's.** Violet marks it wherever it
   shows up, with the agent's name beside it.
6. **Reasons are cheap.** Any decision can take a reason in one optional field,
   committed with Enter.
7. **Numbers are monospaced and come from the server.**

## Narrow screens and public pages

Decided on 2026-10-25 ([plan](../../../docs/deck-builder-v3-plan.md) D17,
D20) and built in phase 4a; phase 4b makes the full app hold up between
720 px and 1024 px.

- **Phones (under 720 px) read and decide.** The deck list, a read-only deck,
  the inbox, and accept/reject with a reason. Nothing else writes. A screen
  that cannot work there says "Open on a larger screen" rather than
  squeezing in. The breakpoint is `PHONE_MAX_WIDTH` in `app/breakpoints.ts`
  (CSS: `@media (max-width: 719px)`); the other two max-width queries are 959 px and 1099 px.
- **720 px and wider is the full app.** The sidebar becomes a drawer under
  960 px (`DRAWER_BREAKPOINT`), and the deck panel becomes a bottom sheet under
  1100 px (`PANEL_BREAKPOINT`). At 960–1099 px the rail remains visible while
  the main content scrolls the window. Desktop panes remain pinned at 1100 px.
- **Touch replaces hover.** On touch (`hover: none`) the hover card never
  opens; a tapped card opens `CardSheet`, a bottom sheet on a phone. Tap
  targets are at least `--tap` (44 px): `Button size="touch"`,
  `IconButton size="touch"`, and list rows grow to it under 720 px. The
  dense 24px rows of principle 4 are a desktop rule.
- **Nothing scrolls sideways at 375 px.** Names wrap (`overflow-wrap:
  anywhere`), action rows wrap, two-column grids collapse, and a table
  scrolls inside its own box, never the page.
- **Public pages show the commander's art** in a showcase band above the
  header (`DeckArt variant="showcase"`, see Art), credited inside the crop,
  and as the link preview image. Without a stored artist the art is left
  out. The card list itself stays text.
- **Sharing has no colour.** Public is the globe icon, unlisted the link
  icon, both grey: visibility is not one of the meanings below.
- **The playtester is desktop-only.**

### The patterns

| Pattern | Where | Notes |
| --- | --- | --- |
| `Dialog placement="sheet"` | `ui/` | Bottom edge, full width, `--sheet-max-h`; header one tap tall |
| `CardSheet` | `mtg/` | The hover card's facts in a dialog, opened by tap; sheet on phones |
| `ReadOnlyDeck` | `features/deck` | List beside price, power, brief, checks, decisions; one column under 720 px |
| Shared deck header | `features/public` | Art, name, owner, commander, target; Copy link, Export, Fork, Compare |
| Share dialog | `features/deck/dialogs` | Segmented visibility with its consequence in words; link; publish decisions |
| Phone shell | `features/phone` | 48 px top bar; one decision per card with reason, Accept, Reject; Accept all |

## What each colour means

| Token | Meaning | Examples |
| --- | --- | --- |
| `--agent` violet | An agent wrote or proposed it | proposal banner, memory chips, agent names, "waiting on you" |
| `--add` green | Coming in, passing, accepted | proposed add, a check that passes, Accept |
| `--cut` red | Going out, failing, rejected | proposed cut, a rule violation, Reject |
| `--warn` amber | Off target, missing, provisional | slot under target, incomplete deck |
| `--sel` blue | The owner's selection, focus and primary action | selected rows, search and tag matches, focus ring, focused field; `--action` aliases it for primary buttons |
| `--identity-*` | That deck’s colour identity | thin stripe on its card or header only |
| greys | Everything else | structure, text, neutral chips, deck lifecycle |

Mana colours (`--mana-*`) appear in symbols and the pip chart. D21 also
permits `--identity-*` aliases on a deck's own thin stripe, at
`--identity-opacity`; colourless uses `--identity-grey`. A split gradient
represents multiple colours. These accents never tint text or controls and
never replace semantic colours.

Deck lifecycle (Paper, Theorycrafted, Archived) is a state, not a meaning, so
its labels are grey and the word carries it: All decks pills, sidebar labels
and the deck header's lifecycle control. The `--lifecycle-*` aliases resolve to
`--muted` on `--raised`, keeping green, blue and amber for accept/pass,
selection and warnings. Sidebar labels appear only for Theorycrafted and
Archived decks.

The palette follows [T3 Code’s redesign dark theme](https://github.com/pingdotgg/t3code/blob/main/apps/web/src/index.css)
(`html.dark[data-redesign]`): `--bg` is #000, including the sidebar;
`--surface` and `--overlay` are #111 (neutral-950 with about 3% white),
with #1a1a1a for `--raised` selected/control surfaces. Popovers stay opaque.
Hover is 4% white over the existing surface. Rules are 8% white, control
borders 11%, hover borders 18%; dim decorative elements with transparency.
Inputs use the near-black surface and control border. Primary buttons use
`--action` blue with black ink (8.30:1; 9.64:1 on hover); accept/reject
and agent actions keep their semantic colours.

Text is neutral #f5f5f5 / #d4d4d4 / #a3a3a3 / #9c9c9c. Conservative
minimum ratios, including two stacked 6% semantic tints over a hovered
raised surface: text 11.51:1, soft 8.47:1, muted 4.97:1, faint 4.57:1.
The semantic tints use 32% borders. Violet, green, red, amber and blue text
also pass AA across that envelope; exact measurements are in tokens.css.
Red is slightly lighter to keep nested reject controls readable.

## Tokens

All in `src/design/tokens.css`: surfaces, text, the five meanings (each with
`-bg` and `-line`), mana, type sizes, a spacing scale (`--s-1` … `--s-8`),
radii, control heights, layout widths, elevation, stacking and motion.

Component CSS uses tokens only. If you reach for a literal, add a token.

Text has four tiers, brightest first: `--text`, `--text-soft`, `--muted`,
`--faint`. Each passes WCAG AA (4.5:1) on every surface and tint; the measured
ratios sit beside the tokens. Stacking runs `--z-sticky` < `--z-pop` <
`--z-dialog` < `--z-hover` < `--z-toast`; a popover inside a dialog is
portalled into it, so it needs no higher layer. `--measure-wide` is the wide
page and first-run width; `--tablet-bar-h` the drawer's menu bar.

`design/base.css` holds the one global utility, `.visually-hidden`, for text
only assistive technology reads: a column header with no visible label, a
severity before a message, the rest of a link's name.

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
- The owner's choices are decisions: they accept, reject or decide. Never
  "rule" or "ruling", which in Magic mean the game's rules and card rulings.
  `web-architecture.test.ts` checks every string in `features/` and `app/`.
- One name per action. Opening the create dialog is "New deck" everywhere.

## Components

`ui/` holds the generic primitives and `mtg/` the card-specific ones. The
inventory, with every variant and state, is the style guide page at
`/design`; keep it complete, because it is how a new page is made to match.

<!-- inventory:start -->
Everything below is exported from `ui/index.ts`. None of it knows about Magic
or about this app.

| Component | For | Variants |
| --- | --- | --- |
| `Button` | Any action | `default` `ghost` `primary` `accept` `reject` `agent`; `sm` `md` `touch`; `kbd` hint |
| `IconButton` | An action with only an icon; `label` is required | `sm` `md` `touch` |
| `Icon` | The 17 line icons (inbox, decks, memories, activity, profile, settings, back, close, check, plus, chevronDown, undo, redo, search, more, globe, link) | `size` |
| `Kbd` | A key cap | |
| `Chip` | A small tag or filter; a button when given `onClick` | tone `neutral` `agent` `warn` `add` `cut`; `dashed` for unconfirmed |
| `Badge` | A count pill | `neutral` `agent` `warn` |
| `Pill` | A selectable filter in a row of filters; `aria-pressed` carries the state | `on` |
| `Dot` | A 6px status mark | `neutral` `agent` `warn` `add` `cut`; `pulse` |
| `Tabs` | Switching panels, underline style, arrow keys | per-tab `badge`, `tone: "agent"` |
| `Segmented` | Choosing one of a few views | |
| `Menu` | A text or icon button that opens a list of actions | trigger `variant` and `size`; `icon`; `align`; item `tone: "cut"`, `disabled` |
| `ContextMenu` | Pointer-positioned actions for a right-click or keyboard menu request | disabled items, separators, arrow navigation, focus restoration |
| `TextField` | One-line input | `fieldSize` `md` `lg`; `invalid` |
| `TextArea` | Multi-line input | `autosize` |
| `Select`, `Checkbox` | Native controls, restyled | |
| `MultiSelect` | Searchable multiple-selection listbox with removable selected chips | controlled `value`, async `onChange`, empty state, `compact` trigger summary, dropdown `footer` |
| `Field` | Label, control and hint, wired together | |
| `Meter` | A bar toward a limit | `neutral` `agent` `warn` `add`; clamps to 0-100% |
| `Box` | A bordered container | tone `neutral` `agent` `warn`; `dashed`; padding `none` `sm` `md` |
| `Eyebrow`, `Section` | The small uppercase label, and a titled group with an `aside` | |
| `Stack`, `Row`, `Spacer` | Layout on the spacing scale `--s-1` to `--s-8` | `gap` 1-8; Row `align`, `justify`, `wrap` |
| `Text` | Text with a tone, size and weight from the tokens | tone: 8; size `xs` to `xl`; `mono`; weight 3 |
| `Banner` | A full-width strip under a header | `agent` `warn` `error` `neutral`; `actions` |
| `ErrorNote`, `EmptyState`, `Loading` | An inline error, "nothing here yet", and a loading line | EmptyState `heading` `h1` `h2` when it is all the page has |
| `Dialog` | A modal: focus trap, Esc, backdrop click, focus restore, `data-autofocus` | width `md` `lg`; placement `center` `sheet` `drawer`; `footer` |
| `ConfirmDialog` | A question with Cancel and a confirm button | tone `default` `danger` |
| `Explainer` | One sentence and a Learn more popover | closed, open, keyboard focus, close/Escape and focus return; `defaultOpen` (style guide only) |
| `Popover`, `placeNear` | A panel anchored to an element, kept inside the viewport | placement `below` `right` `above` `left`; `align` |
| `ToastProvider`, `useToast` | One toast at a time, bottom centre, with an optional action | `duration`; `action` with `kbd` |
| `DiffView` | Added and deleted lines | line kind `same` `add` `del` |
| `Table`, `ClickableRow` | Plain tables with small uppercase headers; rows that open something | |
| `CounterControl` | Visible increment/decrement controls and a numeric readout | labeled, disabled |
| `KeyValue` | Two-column label and value list | |
| `ResizeHandle` | A draggable edge inside the element it resizes; arrow keys, Home, End, double-click to reset | `edge` `left` `right`; caller owns `value`, `min`, `max` |
| `Page`, `PageHeader`, `SplitPane` | Screen frame with a sticky header and scrolling body; list and detail columns | Page width `md` `lg` `full` |
| `useHotkeys` | Declare a screen's shortcuts (`"j"`, `"shift+a"`, `"mod+k"`); ignored while typing except Escape | `enabled`, `allowInInputs` |
| `pressesFocusedControl` | Whether Enter or Space would press the focused button or link, so a screen shortcut on that key stands down | |
| `useLocalStorage` | State kept in localStorage | |
| `tokenPx` | A size token as a number, where script has to do arithmetic with one | |
| `useNearView` | Whether an element is within a margin of view in its own scroll container (or the window); latches true. For deferring downloads | `margin` |
<!-- inventory:end -->


## Manual table extensions

The playtester deliberately uses larger rules text (`--text-rules`, 15px,
`--leading-rules`, 1.6) without changing the dense deck-list typography.
`mtg/table` contains text cards, zone containers, player summaries and card
inspection with host-supplied contextual tools. Full, compact, hidden, selected,
tapped, targeted and disabled observations are shown at `/design`.

The table surface (`--table-bg`) shares the true-black canvas so near-black
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

## Phase 4b audit and patterns (2026-10-25)

Applied the installed `design:design-system` and `design:ux-copy` skill
checklists by reading their SKILL.md files directly; this runtime has no Skill
tool. The audit covered tokens.css, ui/, and the existing /design examples.
The system already had semantic colors, touch sizing, a focus-trapping Dialog,
and CardSheet. Gaps: unreadable faint text; checkbox-group semantics for a
multiple choice control; no explainer or drawer pattern. Existing
principles are preserved. On tablet only, the owner may explicitly open a
sheet over the list (principle 1's existing requested-dialog exception).

- **Contextual details (owner decision, 2026-10-27):** revisions belong in
  History and card decisions; memory/assessment revision metadata goes in
  titles. Deck headers have no revision chip. Budgets and usage belong only
  in Settings. Secondary slots sit directly below the primary slot as a
  compact field; list rows do not repeat them. Agent review status is plain
  in decisions and the inbox. Ownership evidence appears in the card panel
  only when present, with its basis and attribution. No shared disclosure.
- **Explainer:** a sentence about the concept in the task's context, followed
  by “Learn more” with the concept in its accessible name. Click, Enter or
  Space expands a labelled popover region; Close, Escape or outside click
  dismisses it. Opening focuses the explanation; a nested popover stays within
  its parent dialog’s focus trap and Escape dismisses only that popover. Focus returns to its trigger. Use it for lifecycle, slots vs
  tags, brief, bracket, proposals, and memories. It does not change data.
- **Drawer:** `Dialog placement="drawer"`, `--drawer-w`, full viewport height,
  left edge, internal scrolling. Same focus trap/restore, Escape, close and
  backdrop rules as Dialog. At 720–959 px it contains the normal sidebar.
- **Bottom sheet:** existing `Dialog placement="sheet"`, now also the full
  deck panel under 1100 px. The list stays in the window; the requested panel
  has `--sheet-content-h` and scrolls internally. Header stats, tabs and card
  selection open it; Close/Escape returns to the list. Playtest requires 1100 px.
- **Listbox:** MultiSelect's filtered options have `role=option` and
  `aria-selected` inside an `aria-multiselectable` listbox. Up/Down navigates
  search and options; Space/Enter toggles; Escape/Done returns to the trigger.
  The listbox shows empty and no-match states as a disabled option (so its
  required option role remains present), and disables writes while busy.
- **Card inspection:** `?` on a focused card row opens the same CardSheet as
  tapping its mana cost. Inline card names without a host action open it too.
  The modal is named, traps focus, closes with Escape and restores focus.
- **Text and motion:** the neutral text tiers stay in order and pass AA on
  tinted rows (see Tokens for the current palette and measured minima).
  Filtered-out cards are not dimmed: their names still need to be read, and the matches carry
  the selection tint instead. Global reduced-motion zeroes `--fast` and
  disables transitions/animations. Inter is removed; the system font stack
  avoids a third-party font request.

Copy follows “Writing in the UI”: sentence case, outcome-led actions, useful
empty states, no invented card facts. Agent-sensitive help, subtitles and
empty states use `useVoice(first, connected)`, driven by the server's first
agent-write flag; neutral loading, errors and factual hints are shared by both
voices. New-account copy talks about decks and plans before agents. The landing
has one optional Settings → Connect an agent pointer. The sample is labelled,
private, and every synthetic write names sample-agent. “Archived” describes a
lifecycle; “decisions” describes the owner's choices, avoiding MTG format and
card-ruling terminology. Stored user prose is never renamed.

The live inventory at /design includes contextual metadata and compact slot
fields, both overlays, closed and open explanations, selected/empty/busy
listboxes, a plain archived title, accessible faint text and motion guidance.


## Art (D21, 2026-10-26)

The owner's brief: colour and real card art without distraction; the artist
credit subtle but present; identity tints never overdone.

`mtg/DeckArt` owns crop rendering, credit and reserved space; `mtg/DeckThumbnail`
the rail's small version. Both take stored card facts (`domain/art.ts`
`ArtFacts`) and never fetch them. One helper, `creditedArt`, decides what
qualifies: an `https` URL on `cards.scryfall.io` whose parsed path starts
with `/art_crop/`, with a nonblank artist; at most two (a partner pair). No
generated or stock images, bundled images, large card images, constructed
URLs or browser Scryfall API calls. On-demand full card pictures keep their
printed artist and copyright lines.

| Variant | Placement | Band (`--band`) | Loading |
| --- | --- | --- | --- |
| strip | Desktop and phone home cards | `--art-strip-space` | deferred: `load` from `useNearView` (200 px), then eager |
| banner | Owner and phone deck headers | `--art-banner-space`, smaller on tablet/phone | eager |
| showcase | Public deck page | `--art-showcase-space` (176 px), 112 px on phones | eager, `priority="high"` (`fetchpriority`) |
| backdrop | Sign-in and empty-account landing only | `--art-backdrop-space` | eager; one land per visit, no rotation |
| thumbnail | Sidebar rail, including the drawer | `--art-thumb` (16 px), the icon column | deferred: `useNearView` in the rail, `fetchpriority=low` |

- **The art is a band above the content.** Content starts where the band
  ends, on `--art-overlay-text` or darker, and the crop fades into the
  surface below it. Nothing sits on bright art except the credit pill. The
  frame's size comes from the band, never the image, so loading moves nothing.
- **The credit is inside its crop, bottom-right**, on a small
  `--art-overlay-text` pill in `--text-xs` `--muted`. Partners get one per
  crop half. Where the commander's name is visible beside the art (roster
  cards, deck headers, the public page) it reads "Art by <artist>"; where no
  card is named (backdrops, the rail tooltip) "Art by <artist> · <card>".
  Double-faced cards use the front-face name. A failed or not-yet-loaded
  image takes its credit with it; the empty frame keeps its space.
- **The rail thumbnail is decorative** (`aria-hidden`), so the link's name is
  the deck name. Its credit is the native tooltip. It sits at
  `--art-thumb-opacity` until its row is current or hovered, replaces the
  neutral dot (a deck waiting on the owner keeps its violet dot on the
  thumbnail's corner), and the row's gaps give back its width: names
  truncate exactly where they did before thumbnails.
- **Roster cards are one link.** The deck name's link is stretched over the
  card; the commander links and the actions menu sit above it. Cards fill
  their grid row with the stats pinned to the bottom.
- **Backdrops show the land unblurred** in a band at the top of a solid
  surface panel; the form or first-run choices sit on the surface below.
- **Overlays derive from `--bg`** (`color-mix`), so a palette change carries
  them. Measured with WCAG 2.x sRGB luminance over pure white art (94%
  black overlay): muted credit 7.58:1, faint text under any band 6.97:1 or
  better. Strip, banner, showcase and backdrop share this floor; no overlay
  strength changes are needed for the black canvas.
- **Identity is a 2 px stripe** at `--identity-opacity`, split for several
  colours, grey for colourless. Black uses the darker half of its pip
  (`--identity-b`) so it never reads as white. No identity fills, tints of
  text or controls.
- **The desktop banner folds** while the deck list (only it, not the panel)
  is scrolled past `--art-collapse-threshold`, unless the list is too short
  to stay folded. Padding, height and opacity animate over `--base`, which
  is 0 under reduced motion; the credit hides with the art. The fold exists
  only at 1100 px and wider: `useArtCollapse` drops it below that, and the
  CSS ignores `data-collapsed` there. A fold and unfold together stay under
  0.1 cumulative layout shift. Narrower headers scroll away with the window.

Art does not appear in inbox, activity, memories, Settings, dialogs or the
playtester content. The rail can still show thumbnails while those screens
are open.

History: phase 4c added art (D21); its review moved credits into the crops,
made roster cards whole links again, unblurred the backdrops and restored the
public showcase band.

## Sidebar and Profile navigation

All decks comes first, then Inbox, Memories, Activity, Profile and Settings. Deck links
have a separate “Your decks” heading, with the same icon/text alignment and
row spacing as section links. Agent presence is a quiet summary:
last writer and time, then “Agent context” with the open deck and selection.

At 1100 px and up, Collapse sidebar switches to a 64 px icon rail. Expand
sidebar reverses it. The button exposes expanded state; each link keeps its
accessible name and a title, including deck names, and the inbox badge stays
visible. The account preference persists on the server and defaults to
expanded. Failed saves keep the prior state and show an error toast.
The 960–1099 px rail and under-960 drawer deliberately keep visible labels
and have no collapse control; widening restores the saved desktop choice.
The inbox count uses a filled `--agent` pill with `--bg` ink (7.71:1
WCAG contrast) in both layouts. The phone shell is unchanged.

Following review, the owner decided on 2026-10-27 to restore Activity as a
separate sidebar item after Memories. `/activity` has its own Activity title
and current link; `/profile` is a single page with no tabs. A profile draft
survives resizing. The inbox’s unreviewed decision count sits in its padded
header subtitle, aligned with the rest of the page content.
