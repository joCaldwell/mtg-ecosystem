# Design system

A dark, dense tool for long sessions of reading card text and making
decisions. The live reference is the style guide at `/design`; this file is
the rules behind it.

## Principles

1. **The deck list is home.** Everything else comes to it: a side panel, a
   banner, a hover card. Nothing covers the list except a dialog you asked for.
2. **Colour means something or it is neutral.** The canvas is true black and
   the sidebar just above it. Near-black surfaces lift slightly; transparent white hairlines
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
- **720 px and wider is the full app.** The sidebar lives in a drawer below
  1100 px, with the current page and Inbox count in its top bar. The deck panel
  becomes a bottom sheet under 1100 px (`PANEL_BREAKPOINT`), while the main
  content scrolls the window. Desktop panes remain pinned at 1100 px.
- **Touch replaces hover.** On touch (`hover: none`) the hover card never
  opens; a tapped card opens `CardSheet`, a bottom sheet on a phone. Tap
  targets are at least `--tap` (44 px): `Button size="touch"`,
  `IconButton size="touch"`, and editor rows and controls grow to it through
  1099 px, including fine pointers and freshly imported decks. The dense 24px rows of principle 4 are a desktop rule.
- **Nothing scrolls sideways at 375 px.** Names wrap (`overflow-wrap:
  anywhere`), action rows wrap, two-column grids collapse, and a table
  scrolls inside its own box, never the page.
- **Public pages show the commander's art** in a showcase band above the
  header (`DeckArt variant="showcase"`, see Art), credited inside the crop,
  and as the link preview image. Without a stored artist the art is left
  out. The card list itself stays text.
- **Sharing has no colour.** Public is the globe icon, unlisted the link
  icon, both grey: visibility is not one of the meanings below. The deck
  header's button always reads "Share"; the icon shows the state and the
  accessible name includes it, so publishing never wraps the header.
- **The playtester is desktop-only.**

### The patterns

| Pattern | Where | Notes |
| --- | --- | --- |
| `Dialog placement="sheet"` | `ui/` | Bottom edge, full width, `--sheet-max-h`; header one tap tall |
| `CardSheet` | `mtg/` | The hover card's facts in a dialog, opened by tap; sheet on phones |
| `ReadOnlyDeck` | `features/deck` | List beside price, power, brief, checks, decisions; one column under 720 px |
| Shared deck header | `features/public` | Art, name, owner, commander, target; Copy link, Export, Fork, Compare at desktop button size; signed out, a secondary "Sign in to fork or compare" button. The owner's own link previews the signed-out view, with icon-only Back to the deck in its note |
| Share dialog | `features/deck/dialogs` | Segmented visibility with its consequence in words; link with Copy link and a secondary "Preview public page"; publish decisions |
| Phone shell | `features/phone` | 48 px top bar; one decision per card with reason, Accept, Reject; bulk actions in Review all decisions |

## What each colour means

| Token | Meaning | Examples |
| --- | --- | --- |
| `--agent` violet | An agent wrote or proposed it | proposal banner, memory chips, agent names, "waiting on you" |
| `--add` green | Coming in, passing, accepted | proposed add, a check that passes, Accept |
| `--cut` red | Going out, failing, rejected | proposed cut, a rule violation, Reject |
| `--warn` amber | Off target, missing, provisional | slot under target, incomplete deck |
| `--sel` blue | The owner's selection, focus and primary action | selected rows, focus ring, focused field; `--action` aliases it for primary buttons |
| `--identity-*` | That deck’s colour identity | thin stripe; a restrained tint on its gallery fallback |
| greys | Everything else | structure, text, neutral chips, deck lifecycle |

Mana symbols are the real ones from the cards (owner decision, 2026-10-29,
replacing the lettered pips of 2026-10-28): each pip is Scryfall's own SVG
for that symbol from `svgs.scryfall.io`, in a fixed box at the pip's size.
Hybrid, monocolour hybrid and Phyrexian pips are Scryfall's combined
drawings, never our split halves. Each pip is one image named by the spoken
symbol ("black or green"); the drawing inside it is silent. Until the SVG
loads, or when it cannot (offline local mode), the box shows a plain disc in
the `--mana-*` colours with no letter; black keeps its pale ring, and a
generic cost keeps its numeral in mono bold `--mana-ink`, centred on its
digits rather than its line box. Nothing moves when the image arrives. A
colour identity (`mtg/ColorIdentity`) is the same symbols at
`--identity-pip` (12px), one image named "Colour identity: …", so roster
cards, public profiles and collection statistics never rely on hue alone.
The `--mana-*` colours also draw the pip chart and the named collection
identity bars; the collection's black bar uses a solid charcoal alias. D21 also
permits `--identity-*` aliases on a deck's own thin stripe at
`--identity-opacity`, plus an 18% surface tint on its gallery fallback;
colourless uses `--identity-grey`. A split gradient
represents multiple colours. These accents never tint text or controls and
never replace semantic colours.

Deck lifecycle (Paper, Theorycrafted, Archived) is a state, not a meaning, so
its labels are grey and the word carries it: My decks metadata, sidebar labels
and the deck header's lifecycle control. The header shows lifecycle as a
pill, not a tab row (owner decision, 2026-10-28): the owner opens it as a
single-choice menu, moving to Archived asks first, and read-only headers show
the plain chip. "Who can edit?" is the menu's last item, after a rule; it
opens the lifecycle explanation beside the pill, not a link in the header. The `--lifecycle-*` aliases resolve to
`--muted` on `--raised` (the roster uses `--text-soft` for essential lifecycle
labels), keeping green, blue and amber for accept/pass,
selection and warnings. Following repeated blind review (2026-10-28), the sidebar keeps
Theorycrafted and Archived below the deck name. Collapsed links retain those
labels in their accessible names and tooltips. Paper stays implicit.

The palette follows [T3 Code’s redesign dark theme](https://github.com/pingdotgg/t3code/blob/main/apps/web/src/index.css)
(`html.dark[data-redesign]`): `--bg` is #000;
`--surface` and `--overlay` are #111 (neutral-950 with about 3% white),
with #1a1a1a for `--raised` selected/control surfaces. Popovers stay opaque.
The sidebar departs from it: `--sidebar-bg` is #0a0a0a behind a 16% white edge.
Hover is 4% white over the existing surface. Rules are 8% white, control
borders 11%, hover borders 18%; dim decorative elements with transparency.
Inputs use the near-black surface and control border. Primary buttons use
`--action` blue with black ink (8.30:1; 9.64:1 on hover); accept/reject
and agent actions keep their semantic colours.

Text is neutral #f5f5f5 / #d4d4d4 / #b0b0b0 / #a6a6a6 (muted and faint
raised on 2026-10-29: meta lines read as dim grey on black). Conservative
minimum ratios, including two stacked 6% semantic tints over a hovered
raised surface: text 11.51:1, soft 8.47:1, muted 5.79:1, faint 5.15:1;
on `--bg` and `--raised`, muted 9.68 and 8.03:1, faint 8.63 and 7.15:1.
The dimmer `--disabled-ink` (#808080) is for disabled controls only.
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
  `--text-xs`, also 12px since 2026-10-28: 11px mid-grey meta on black was
  hard to read. `--text-2xs` (10px) is only for key caps.
- The sign-in product name `--text-wordmark` (24px).
- Page titles `--text-xl`, weight 650. Card and section titles `--text-lg`.
- Eyebrows (small section labels): `--text-xs`, uppercase, letter-spaced,
  `--faint`.
  A card proposal's label is the one exception: `ProposalEyebrow` keeps
  sentence case so handles and agent labels read as written.
- Counts, revisions, mana values, times: `--font-mono` with tabular numerals.
  It is one stack of platform UI monospace faces (no downloads); named Linux
  faces precede the bare generic, which is often a Courier clone there.
  `code`, `kbd`, `pre` and `samp` use it too. Mono beside sans text aligns by
  baseline, not centre, since the faces' vertical metrics differ.
  Deck prices, slot totals and header metadata use at least `--text-sm` (12px).

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

## Brand

`app/BrandMark` is the product mark: three cards fanned from one point, drawn
as an inline SVG in `currentColor`, each card cut back from the one in front
so the stack reads in one colour at 16 px. It is decorative (`aria-hidden`);
the wordmark "Deck Builder" or the surrounding link carries the name. Beside
the wordmark it is inline and `--brand-mark-h` (`1cap`) tall, standing on the
baseline at cap height: the sidebar and public header (`--text-lg`), and
centred above the sign-in card (`--text-wordmark`), whose first line is the
tagline. Alone (collapsed rail, phone bar) it is `--brand-mark-rail` (16px).
The favicon in `web/index.html` is the same geometry as a data URI, dark on
light browser chrome and light on dark. No raster logo, font or generated art.

## Components

`ui/` holds the generic primitives and `mtg/` the card-specific ones. The
inventory, with every variant and state, is the style guide page at
`/design`; keep it complete, because it is how a new page is made to match.

<!-- inventory:start -->
Everything below is exported from `ui/index.ts`. None of it knows about Magic
or about this app.

| Component | For | Variants |
| --- | --- | --- |
| `Button` | Any action | `default` `ghost` `primary` `accept` `reject` `agent`; `sm` `md` `touch`; `kbd` hint; `reach` (invisible padding to `--tap` tall, for compact Edit controls); disabled is one neutral look for every variant: no fill, `--disabled-ink` label and `--disabled-line` border, visible but clearly unavailable |
| `ButtonLink` | A link drawn as a Button, for an action that navigates | `default` `ghost` `primary`; `sm` `md` `touch` |
| `IconButton` | An action with only an icon; `label` is required | `sm` `md` `touch`; tone `danger` (grey bordered, red only on hover or focus: row Delete and Remove) |
| `Icon` | The 19 line icons (inbox, decks, memories, activity, profile, settings, back, close, check, plus, chevronDown, undo, redo, search, more, globe, link, trash, warning) | `size` |
| `Kbd` | A key cap, `--text-2xs` (11 px) in `--text-soft` | |
| `Chip` | A small tag or filter; a button when given `onClick` | tone `neutral` `agent` `warn` `add` `cut`; `dashed` for unconfirmed |
| `Badge` | A count pill | `neutral` `agent` `warn` |
| `Pill` | A selectable filter in a row of filters; `aria-pressed` carries the state | `on` |
| `Dot` | A 6px status mark | `neutral` `agent` `warn` `add` `cut`; `pulse` |
| `Tabs` | Switching panels, underline style, arrow keys | per-tab `badge`, `tone: "agent"` |
| `Segmented` | Choosing one of a few views | |
| `Menu` | A text or icon button that opens a list of actions; a `pill` trigger shows a current state and its single-choice list | trigger `variant` and `size`; `icon`; `pill`; `align`; `triggerRef`; item `tone: "cut"`, `disabled`, `checked` (menuitemradio with a tick; an item without it stays a plain action), `separated` (a rule above it) |
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
| `Dialog` | A modal: focus trap, Esc, backdrop click, focus restore, `data-autofocus` | width `md` `lg` `wide`; placement `center` `sheet` `drawer`; `footer` |
| `ConfirmDialog` | A question with Cancel and a confirm button | tone `default` `danger` |
| `Explainer` | Specifically labelled help for unfamiliar concepts | closed, open, keyboard focus, close/Escape and focus return; `defaultOpen` (style guide only); `control` (opened by a menu item, beside its anchor, no trigger of its own) |
| `Popover`, `placeNear` | A panel anchored to an element, kept inside the viewport | placement `below` `right` `above` `left`; `align` |
| `ToastProvider`, `useToast` | One toast at a time with an optional action, and a second after it (wrapping below the message where narrow); bottom left at 1100 px and wider (over the sidebar footer, clear of the deck list and the panel's actions), bottom centre below | `duration`; `action` with `kbd`; `secondaryAction` |
| `DiffView` | Added and deleted lines | line kind `same` `add` `del`; `foldSame` (each unchanged run one closed native disclosure, "4 unchanged lines"); Markdown list lines draw a bullet, not their hyphen |
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
remain visible. An empty battlefield lets the full-text hand take its natural
height; its three divider rows stay available for drops. A populated battlefield
or open zone viewer restores the regular table allocation. Hand rules can scroll
vertically even in short windows. The hand, battlefield, library drawer and inspection panel scroll
independently; fonts never shrink as a table grows. Selection has a blue border,
tapping has an explicit state/action and a dashed edge, and temporary inspection
never changes the pinned selection or an unfinished observation's target.

Each battlefield lane owns its full visible bounded box as a drop target,
including its blank upper area and gutters. The first lane grows to fill spare
table space while its cards stay bottom-aligned. Drag highlighting follows that
same box, remains stable over child cards, and clears on drop or cancellation.

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
- **Explainer (owner direction, 2026-10-27):** only for unfamiliar concepts,
  with a specific question or label such as “Who can edit?” or “How proposals
  work”. Do not add generic “Learn more” buttons or repeat prose for obvious
  controls. Essential warnings, permissions and destructive consequences stay
  visible at the decision point. Click, Enter or Space expands a labelled
  popover region; Close, Escape or outside click
  dismisses it. Opening focuses the explanation; a nested popover stays within
  its parent dialog’s focus trap and Escape dismisses only that popover. Focus returns to its trigger. Use it sparingly for lifecycle consequences, slot targets and proposal
  mechanics. It does not change data.
- **Drawer:** `Dialog placement="drawer"`, `--drawer-w`, full viewport height,
  left edge, internal scrolling. Same focus trap/restore, Escape, close and
  backdrop rules as Dialog. At 720–1099 px it contains the normal sidebar.
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
  Find-in-deck and the tag filter hide nonmatching cards (owner decision,
  2026-10-28, replacing the earlier muting): groups and the maybe list with no
  match disappear, the rest count "2 of 14", and matches need no highlight
  because every row shown is one. Global reduced-motion zeroes `--fast` and
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

| Variant | Placement | Reserved frame | Loading |
| --- | --- | --- | --- |
| strip | Desktop/phone roster, public profile, style guide | `--art-aspect` (626/457) gallery frame | deferred: `load` from `useNearView` (200 px), then eager |
| editor | Owner’s desktop/tablet deck header | `--art-editor-h` (200–240 px by viewport height); tablet 256 px | eager |
| banner | Phone deck header | `--art-editor-phone-h` (224 px minimum) | eager |
| showcase | Public deck page | `--art-showcase-space` (176 px), 112 px on phones | eager, `priority="high"` (`fetchpriority`) |
| backdrop | Sign-in and empty-account landing only | `--art-backdrop-space` | eager; one land per visit, no rotation |
| thumbnail | Sidebar rail, including the drawer | `--art-thumb` (16 px), the icon column | deferred: `useNearView` in the rail, `fetchpriority=low` |

- **Roster captions sit on the art, without a panel edge.** The `strip`
  variant reserves `--art-aspect` (626/457), roughly 280 px tall at 1440.
  Its image runs through the full tile. The caption's eased, multi-stop fade
  begins 104 px above its text, reaching 58% black by the title, 64% before
  commanders and 74% before metadata. The caption keeps the painted detail:
  a masked 8 px backdrop blur was compared in captures and removed because
  the plain gradient preserved more of the painted detail.
  Legibility never depends on backdrop support.
  The 21 px deck name leads; commanders are 12 px, or 14 px on phones.
  Owner roster metadata is 14 px with 16 px symbols; shared profile tiles
  retain their existing metadata sizing. Long titles and partner names wrap without truncation; unusually
  long captions can grow the tile. Grid rows stretch neighbours to match.
- **Gallery credits are unboxed text inside each crop, bottom-right.**
  They use `--text-xs` `--text-soft` on the lower caption gradient, with
  their own reserved line below the metadata. At rest, the tile token
  `--art-gallery-credit-opacity` is 0.22: intentionally barely there and
  exempt from text contrast by owner direction. Hover-capable tile hover or
  focus within raises the credit to full opacity; touch at rest stays quiet. Partner crops split the frame
  equally and each keeps its own credit. The artist name is visible; "Art
  by" stays in the accessible text. Editor and phone headers use the same
  unboxed, low-emphasis rule; showcase and backdrop retain their existing
  credit pills and band fades, and backdrops name the card too. Double-faced
  cards use the front face. Failed or deferred images hide their credits and
  retain the frame.
- **Gallery interaction is visible without motion.** Hover and focus within
  brighten the art with an 8% neutral highlight and lighten its border. The art also zooms gently
  only with no reduced-motion preference. Desktop menus reveal on tile hover
  or keyboard focus within; their translucent, blurred background has its own
  contrast floor. Its tile-specific hover/focus rule overrides the generic
  IconButton hover: 64% black at rest, 74% on hover/focus/open, 82% while
  pressed, with brighter ink and border in the active states. Over white
  art those icon contrasts are 4.53:1, 9.17:1 and 12.47:1 respectively. Touch keeps the menu visible with a 44 px hit area. The
  owner tile's commander names show a dotted underline 2 px below the text
  only on the name's own hover or focus on hover-capable devices. Touch has
  no underline, and gets its hit target from padding around the text.
- **The My decks frame grows only at ultrawide sizes.** Its roster uses
  three desktop columns, including at 1920, so six decks form two full rows.
  Starting at 2200 px the page alone grows from 1440 toward 1940 px, leaving
  three approximately 620 px tiles at 2560, close to the crops' native width.
  Statistics share that frame. Tablets keep two columns and phones one.
- **No art still has a place in the gallery.** Missing commanders, unknown
  artists and failed images retain the same frame, with a restrained identity
  tint over the near-black surface (neutral for colourless). Identity never
  tints readable text, controls or semantic states.
- **The rail thumbnail is decorative** (`aria-hidden`), so the link's name is
  the deck name. Its credit is the native tooltip. It sits at
  `--art-thumb-opacity` until its row is current or hovered, replaces the
  neutral dot (in the collapsed rail a deck waiting on the owner keeps a
  violet dot on the thumbnail's corner; expanded rows show the Inbox's violet
  count badge at their end instead), and the row's gaps give back its width: names
  truncate exactly where they did before thumbnails.
- **Roster cards are one link.** The deck name's link is stretched over the
  card; the commander links and the actions menu sit above it. Cards fill
  their grid row with the caption anchored to the bottom. The whole-link focus
  ring runs inside the tile. The phone roster reuses `DeckCard` and calls the
  page "My decks", while its navigation keeps "Decks".
- **Backdrops show the land unblurred** in a band at the top of a solid
  surface panel; the form or first-run choices sit on the surface below.
- **Overlays derive from `--bg`** (`color-mix`), so a palette change carries
  them. Gallery minima over pure white art, using WCAG 2.x sRGB luminance and
  no blur: title `--text` on at least 58% black is 4.88:1; commanders
  `--text-soft` on at least 64% black are 4.53:1; metadata and
  credits at full opacity `--text-soft` at 74% black are 6.75:1. Amber count text is 4.73:1
  there. Metadata uses the soft tier because muted would be only 3.09:1
  against the commander's 64% floor. The waiting chip is filled `--agent` with
  `--bg` ink, 7.72:1 independently of the art. Showcase and backdrop retain
  `--art-overlay-text` (94% black) and their existing contrast floors.
- **Identity is a 2 px stripe** at `--identity-opacity`, split for several
  colours, grey for colourless. Black uses the darker half of its pip
  (`--identity-b`) so it never reads as white. The gallery and owner header may use a
  subtle identity surface tint; never tint text or controls.
- **The editor opens with a large commander-art header.** A 36 px headline
  and lifecycle pill lead, followed by commander names and identity, estimate
  and power, then counts and the right-aligned Playtest, Share and More.
  The sharp art bleeds in from the right, capped at 626 px (760 px for two
  crops); only a dim 34 px blurred wash stretches across the canvas. Long
  eased horizontal and bottom fades derive from `--bg`. Missing or failed
  art keeps the same black frame with a 12% identity tint. Credits stay
  inside their own crops, unboxed at the gallery's 0.22 emphasis.
- **The header yields to work through native scrolling.** Desktop has a
  short outer page scroll before the existing independent card-list scroll.
  The full header scrolls away to a 56 px compact title, lifecycle and action
  bar. The workspace grows into the uncovered space while an equal end
  spacer shrinks, keeping the native scroll range constant. Rows move only
  through scrolling, the list and panel stay mounted, and list scroll offsets
  are never rewritten by the fold. Panel scrolling alone leaves the header
  open. Find/review/keyboard list reveals also expose the compact workspace;
  clearing a filter restores the saved outer, list and window positions.
  There is no height animation or smooth-scroll dependency, including with
  reduced motion. At 1280 × 800 the header is about 213 px; at 1440 × 900
  it is 240 px. Tablets retain window scrolling and 44 px controls; their
  180 px hero leaves naturally. Commander and estimate text stay on the dark
  side; the status chips and Playtest, Share and More share one row. Add cards
  and Find share the next row at 768 px, with filters beneath. Touch targets
  remain 44 px, and longer content can grow the header. Phones use `banner`, a 26 px title and a
  224 px minimum; proposal notices follow outside the art.
  Each independent list/panel scroller also owns its absolute descendants:
  hidden accessibility suffixes cannot escape into the outer header scroll.
  The outer range is exactly the hero-to-compact height difference, including
  when the list fits. End-of-list space is the normal 16 px gutter; the extra
  64 px bulk-action clearance exists only while the selection bar is present.
- **Header contrast is local to the text.** The canvas fade ends at 75%
  of the header, leaving the exposed face and sky at native brightness. A
  soft bottom gradient starts at 55% of the crop's height and reaches only
  32% black at the bottom; opaque action buttons supply their own contrast.
  Title, commander and metadata blocks each carry a content-sized 64% black
  scrim. Its 104 px horizontal fade and 24 px vertical feather sit outside
  the text, including on phone and tablet; no full-width narrow-screen wash.
  Over pure white art, conservative WCAG sRGB minima are 6.15:1 for the
  `--text` title/commander and 4.53:1 for `--text-soft` metadata. Action labels
  remain at least 15.79:1 through rest and hover; lifecycle soft text is
  11.74:1. Counts retain opaque black chips. Credits remain intentionally
  exempt. Public `showcase` pages and roster `strip` tiles keep their own
  composition.
- **Owner deck text underlines follow the glyphs.** As on roster tiles,
  dotted text decoration sits 2 px below the text at every width. Padding
  supplies the 44 px touch target, with compensating margins and a retained
  row height so the title, metadata and list keep their positions. The same
  treatment covers card references throughout the owner’s phone deck view
  and the editor’s power link, including proposal names and linked prose.



The roster filters form one slim, wrapping toolbar: search and a native
lifecycle select retain explicit accessible names; the count is a status and
Clear filters appears while filtering. Phones place search on its own line
and keep lifecycle and count together. Tile waiting counts come from the Inbox
store's shared map of proposals and document edit groups. It uses the same
`waitingByDeck` helper as the sidebar's local derivation. Until that map loads,
the summary may show a
waiting flag without guessing a count; a loaded zero is authoritative.

Art does not appear in inbox, activity, memories, Settings, dialogs or the
playtester content. The rail can still show thumbnails while those screens
are open.

History: phase 4c added art (D21); its review moved credits into the crops,
made roster cards whole links again, unblurred the backdrops and restored the
public showcase band.

## Sidebar and Profile navigation

My decks comes first, then Inbox, Memories, Activity, Profile and Settings. A
hairline divider separates them from the deck links, which have the same
icon/text alignment and row spacing as section links. Agent presence is a quiet summary:
last writer and time, then “Agent context” with the open deck and selection.

At 1100 px and up, Collapse sidebar switches to a 64 px icon rail. Expand
sidebar reverses it. The button exposes expanded state; each link keeps its
accessible name and a title, including deck names, and the inbox badge stays
visible. The account preference persists on the server and defaults to
expanded. Failed saves keep the prior state and show an error toast.
The 720–1099 px drawer keeps visible labels and has no collapse control;
widening restores the saved desktop choice.
The inbox count uses a filled `--agent` pill with `--bg` ink (7.71:1
WCAG contrast) in both layouts. The phone shell uses the same badge treatment.

Following review, the owner decided on 2026-10-27 to restore Activity as a
separate sidebar item after Memories. `/activity` has its own Activity title
and current link; `/profile` is a single page with no tabs. A profile draft
survives resizing. The count of decisions no agent has read sits in proposal
history, not the inbox header.

The deck editor keeps specifically labelled help for lifecycle consequences,
slot targets and proposal mechanics. Its add bar uses a plus label and raised surface;
query syntax lives with the results. Deck search stays with the filters.
Workspace and roster metadata is at least 12 px; prices, totals, counts and
lifecycle labels use the soft grey tier unless they convey a semantic state.
Card-panel slot, tag and role fields sit beside the art below the rules.
The art shrinks first, reserving at least 240 px for compact label-and-control rows; their
controls fill that width without clipping the tags chevron. Desktop list
columns use a 250 px minimum and 12 px gaps, fitting three at 1440 px with
the default panel, and falling back to two when the panel leaves less room.
At 1100 px and wider the editor adds columns instead of stretching three,
so very wide windows keep each name near its pips and price. Row prices sit
in a fixed, right-aligned tabular column (`--price-column-min`) with
`--price-gap` after the pips, in the editor, maybe list and shared lists.

Owner UX decision (2026-10-27): inbox proposal details can accept or reject the whole selected proposal with an optional reason; Review in the list remains secondary. A phone inbox opens the selected proposal beneath a compact deck heading. Its individual decisions come first; Review all decisions discloses deck-wide actions and other proposals. All inbox badges hide zero counts and use bright violet with black ink. Profiles pair their document and Edit action with proposed edits; public profiles reuse the My decks card presentation. Import previews use the wide dialog, a bounded change list with an explicit continuation count, and warnings outside that list.

Review refinement (2026-10-27): Shift+Enter retains add-bar results and marks
the added card “maybe list”; its Undo removes that shortlisted card. Phone
review opens the selected proposal first, then shows the next pending proposal
directly when it is resolved. Review all decisions keeps its placement below.

Readability refinement: generic buttons and text fields keep a 44 px minimum
through tablet widths, including portalled import and slot dialogs. Maybe note
prose stays in card details, where full paragraphs and linked cards are readable
without entering edit mode. The list keeps compact import-origin counts.
Card rows are atomic inline flex boxes so long groups
can cross native columns without painting one card over another.

Owner layout direction (2026-10-27): secondary pages use a centered, fluid
application frame up to `--page-wide` (1440 px), with page titles and actions
aligned to the same edges as the content. Reading-width pages center their
shorter measure; full workspace pages retain their full width. The My decks
roster fills this frame with responsive columns. Its Paper overview uses four
comparable panels, followed by full-width, wrapping grids for slot usage and
reused cards. Long slot names remain visible, with their values alongside.

Settings shares that frame: credentials and setup are adjacent on desktop,
with more room for setup snippets; price and memory preferences sit together,
followed by card data and account controls. Secondary Settings copy uses body
size. Memory budget fields come before the optional usage disclosure, whose
scope names use normal case. All scopes and precise values remain available.
These layouts stack on tablets, and phone capabilities remain unchanged.

Owner toolbar direction (2026-10-27): the editor has one aligned desktop
working row. “Add cards” is a raised catalogue field, separated by a rule from
the deck search, compact tag filter, and labelled Group/Sort native selects.
Controls stay adjacent and share a height instead of occupying opposite ends
of the viewport. On tablets the add field takes its own row above those view
controls. The always-available add input retains `/`, Enter, Shift+Enter and
Escape. Search hides nonmatches. Active query, match count ("5 matches"),
Clear filters and Export matches appear in a separate context row only while
filtering; that row stays below tablet navigation while the toolbar scrolls
away. With no matches the list shows one line, "No cards in this deck match
“x”.", and nothing else. Clearing returns the list to where it was scrolled.

Catalogue options must match the current query and colour scope, including the
current commander identity. While a replacement search is pending, show a
Searching status with no actionable old results. Enter and Shift+Enter only
act on current options. Loading, errors and no matches are status content,
not options in the result listbox.

## Desktop reading and details

Shared desktop lists require the same minimum column width as the editor and
allow long names to wrap; readers can identify every card without opening it.
Editor rows retain their single-line density. Card details keep compact aligned
label/value rows, bringing card actions and history into view without a scroll.
Settings keeps the connection method visible and puts detailed commands inside
“Setup instructions”. They start open until an agent has connected and reopen
when choosing a method or creating a token. Explicit disclosure choices survive
settings refreshes. Tokens and connected apps remain directly visible.

Maybe rows omit slot and purpose labels. Their native button covers the entire
row, including blank space, mana and price. Full note prose and its linked-card
controls belong to details so long notes cannot enlarge grid tracks.
Enter and Space open details and ? inspects. Main-list modifier
selection is unchanged; maybe cards do not join bulk operations that assume
deck membership. Slot assignments remain stored.

## Content headers and panel views (owner direction, 2026-10-27)

Application page titles and actions belong to the black content frame, with
the same centered width and gutter as their content. PageHeader has no grey
band, divider or explanatory subtitle. Useful state uses its compact meta slot;
permissions and warnings stay with the relevant decision. Inbox shares that
frame while its list and detail panes keep independent scrolling.

Panel navigation keeps all visible text controls on one line at minimum, default
and wider widths: Review, Brief, Power, Stats, Checks, Memories and History.
The current destination has bright text, a 2 px bar along its foot and
aria-current; the others stay grey, and Review shows waiting decisions only as
its violet count pill. Inspection replaces
these controls in the same header with an icon-only back arrow
(a `--h-back` square hit area, hover label "Back to <view>"), title, status and mana cost inline, without a divider
underneath; its accessible name names the previous view. The body never duplicates the title.
Keep that view mounted, hidden and inert, preserving drafts and scroll; switching
cards retains the first origin. Stats and Checks remain distinct. A named region
follows the selection, including in the shared sheet. Statistics lead with totals
and curve, retain the colour demand/source comparison, and disclose detailed
costs, other faces and source coverage. Use a concise methods legend; mana
source copies do not imply guaranteed output, adequacy or casting probability.


## Visual deck statistics

The My decks Paper overview uses a real section heading at the page title's
visual scale, followed by large monospaced Paper decks, families, mana value
and land count figures. The family explanation remains beneath them. Filled
near-black panels have clearer hairline borders and soft, heavier titles.
Common identities occupy one column. Label/value pairs use short measures or
row rules; slot values sit directly beside their names. WUBRG bars match the
named mana legend, with a pale boundary around Black's dark fill.

Only the headline shows land count. `src/stats.ts` groups types from the front
face, whereas `computed.land_count` counts a card with Land anywhere in its
type line. The front-face Land subset is omitted from the type grid to avoid
presenting those different values as competing land totals. All other figures
and averages still come from the server unchanged.


Deck Stats uses summary metrics, a count-labelled mana curve, paired colour
distributions and neutral type-group bars. The owner authorizes WUBRG colour
fills for these charts, matching named visible legends; Black uses its dark
mana fill with a pale boundary, distinct from an empty ring. Each donut states
its own units and total. Fixed cost symbols and recognized source-colour counts
are different denominators; source colours may overlap. No shared total,
deficit, available-mana or casting-odds implication is made. Chart groups stack
at minimum/default panel widths, pairing only with enough actual content width
for two readable charts and legends. Empty rings show explicit zero states;
mono-colour is a complete ring. Exact non-coloured costs, any-colour output,
alternative faces and incomplete source evidence stay separate in specific
details. Chart labels remain at least the small text size; values need no hover.

## Tokens (2026-10-29)

The tokens and emblems a deck makes close the deck list, after the maybe
list, under a `GroupHeader` ("Tokens" and its count) in the deck's own
columns. `mtg/TokenRow` is one dense line: name, "P/T · type" with
Scryfall's "Token" prefix dropped (or "Emblem"), real colour pips when the
token has colours, and "from N" producing cards. Hover shows the token's own
image through the card hover; `?` inspects; Enter or a click opens
`mtg/TokenDetail` in the side panel (a dialog, a sheet on phones, on a
read-only deck): text, picture with "Art by" in the faint tier, and the
producing cards as ordinary list rows that open their own details. A filter
keeps the tokens a shown card makes ("1 of 4") and hides the section when
none. Until the server's token catalog exists the section says "Token list
not available yet."; a deck with none says "This deck makes no tokens."
Both are one faint line. A token with no stored image shows no picture
rather than looking one up by name.


## My decks and touch polish

The collection summary names its metrics “Average mana value” and “Average
lands per deck”. Common colour identities pair 16 px symbols with written
colour names from `COLOR_NAMES`; all five read “All five colours”. Counts
remain aligned at the row end. “Colours in deck identities” is an allowed
use of mana colours: its named bars count Paper decks including each colour.
Black uses a solid charcoal fill (`--collection-mana-b`), with no pale outline.
Slot averages and deck counts align in a monospaced column at each row’s end.

Owner roster metadata is 14 px, with 16 px identity symbols and a thin black
ring. Its existing caption gradient keeps the same contrast floors over
white art: soft metadata 6.75:1 and amber counts 4.73:1. Commander names have
no underline at rest; on hover-capable devices the name’s own hover or focus
shows its dotted underline. Touch keeps the padded hit area and hover-card
behaviour without an underline. Shared profile tiles retain their own style.
The fixed My decks title gains a 1 px line-colour edge after its content
scrolls, without moving the page frame.

On phones, the lifecycle pill sits beside the deck title and wraps with long
names. “Read deck · N cards” is a full-width secondary grey button.
Proposed brief edits keep their label left-aligned and link to the inbox
through a separate violet-outlined “Review in inbox” button. Tablets have
one “Deck details” entry in the pinned navigation bar; the art header has no
duplicate entry. Coarse tablet pointers omit keyboard hints on Add cards
and the review banner, while fine pointers retain them. The details sheet
still uses the same portal and preserves its current view and drafts.
