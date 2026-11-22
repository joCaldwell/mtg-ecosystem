# Design system

A dark, dense tool for reading card text and making decisions. `/design` is the
live component reference. [ARCHITECTURE](ARCHITECTURE.md) owns package boundaries,
data flow, routes and lifetime; this file owns appearance and interaction.

## Principles

1. **The deck list anchors editing.** Details use the side panel, banner or hover
   card. Only an explicitly requested dialog/sheet may cover it.
2. **Colour carries meaning.** Neutral black surfaces carry structure; features
   do not acquire their own colours.
3. **Text first in the editor.** Images are on-demand references, not card-row art.
4. **Density supports reading.** Desktop rows are 24 px; use space between groups,
   not between each row. Touch rules below take precedence at narrower widths.
5. **Every agent write is visibly an agent's.** Use violet and the agent's name.
   Public readers see generic agent attribution; private names never leak.
6. **Reasons are cheap.** One optional field accepts Enter to commit a decision.
7. **Numbers are monospaced and supplied by the server.**

## Tokens and type

`src/design/tokens.css` owns colours, spacing, sizes, radii, widths, elevation,
stacking and motion. Component CSS uses tokens; inline styles carry runtime
values only. `design/base.css` owns the global `.visually-hidden` utility.

| Meaning | Tokens / treatment |
| --- | --- |
| Agent authorship, proposals, waiting | Violet `--agent` |
| Add, pass, accept | Green `--add` |
| Cut, fail, reject | Red `--cut` |
| Missing, provisional, off target | Amber `--warn` |
| Owner selection, focus, primary action | Blue `--sel`; `--action` aliases it |
| Deck identity | `--identity-*`, restricted to stripe/gallery fallback and permitted charts |
| Structure, lifecycle, visibility | Neutral greys; words/icons convey state |

The canvas is true black; sidebar `#0a0a0a`, neutral near-black surface/overlay
`#111`, raised controls `#1a1a1a`. Popovers are opaque. Transparent white edges,
separators and hovers lift these surfaces without blue greys or milky fills.
Use `--control-line` for meaningful 3:1 control boundaries; decorative rules stay
quieter. Positioned fallback stacking is sticky < popover < dialog < hover < toast;
native overlays use the browser top layer. The palette derives from [T3 Code's redesign dark theme](https://github.com/pingdotgg/t3code/blob/main/apps/web/src/index.css).

Text tiers are `--text`, `--text-soft`, `--muted`, `--faint`; all meaningful text
passes 4.5:1 on its actual surface/tint. Exact colours/measurements live beside
the tokens, including nested tints. Disabled ink is for disabled controls only.
Primary action ink stays black; semantic actions retain their own colours.
Forced Colors preserves outlines and selected/current states.

Body is `--text-md` (13 px at the default root); secondary/meta are at least
12 px. `--text-2xs` is reserved for keycaps. Sizes use rem with root 100%, so user
text preferences scale them. Controls grow with content and have at least 24 px
minimum targets; coarse-pointer entry text is at least 1 rem. Titles use
`--text-xl` at weight 650; sections/cards use `--text-lg`. Eyebrows are uppercase
faint labels; ProposalEyebrow uses sentence case for handles/agent labels.
Numbers, mana values, revisions, timestamps and code use the platform mono stack
with tabular numerals and baseline alignment. Prices/totals use at least 12 px.

`Page` uses centered `md` reading width, `lg` up to `--page-wide` (1440 px), or
uncapped `full`. Headers align h1/actions with content on the black canvas,
without grey bands or explanatory subtitles. Compact metadata carries useful
state; warnings stay by the decision. Avoid a second left-aligned cap inside a
centered page. Prose keeps a reading measure. Tables scroll in their own container.

## Writing in the UI

Use sentence case, outcome-led actions, useful empty states and no exclamation
marks. Say “Accept the edit”, not “OK”. Call owner choices decisions, never
“rules” or “rulings”; preserve user-authored text. Revision numbers belong to agent-facing records.
Use one name per action: “New deck” opens creation. Explain privacy/permissions
where they apply and keep essential/destructive consequences visible.

Help is sparse and specifically labelled for unfamiliar concepts (“Who can edit?”,
slot targets, proposals). No generic Learn more or repeated obvious-control prose.
Agent-sensitive copy uses `useVoice`; first-run voice depends on real agent writes,
while the sidebar reports authenticated contact, including reads. Errors and
factual hints stay neutral. Sample data is labelled, private and attributed to
sample-agent; connecting an agent is optional.

## Responsive layout and navigation

Phones below 720 px read and decide: Home, Play setup, library, read-only decks,
Inbox and proposal decisions. No direct deck editing; unsupported desktop screens
explain the larger-screen requirement. Game preview has its explicit phone fallback.
Public pages are separate responsive readers. Nothing makes the page scroll
sideways at 375 px: wrap names/actions/grids and contain table overflow.

At 720–1099 px the full editor scrolls the window, navigation uses a drawer and the
requested deck panel uses a sheet. At 1100 px+ panes scroll independently. CSS's
approved max-width queries are 719/959/1099 px; see ARCHITECTURE for their distinct
uses. Through 1099 px editor rows/controls have 44 px targets regardless of pointer;
coarse pointers omit keyboard hints, fine pointers retain them. Touch uses card
inspection instead of hover. Respect 200% text scaling.

Navigation is Home, My decks, Play, Inbox, Memories, Activity, Profile, Settings.
A rule separates section links from decks. Activity owns its page title; Profile
has no tabs. Desktop collapse produces a 64 px rail with accessible names, titles,
thumbnail/lifecycle state and Inbox count. The desktop toggle stays in the same
position when the rail collapses or expands. Tablet drawers retain labels; widening
restores the saved preference. Failed preference saves retain the prior state.
The footer names latest authenticated request/time plus confirmed Agent context,
never implies a persistent connection or mistakes reads for writes.

Theorycrafted/Archived have readable neutral lifecycle labels; Paper is implicit
in the sidebar. Collapsed links retain lifecycle in accessible names/tooltips.
My decks pills and header lifecycle are neutral. A single-choice lifecycle menu
confirms archiving and ends with “Who can edit?”; read-only state is a plain chip.
Sharing stays neutral: globe/public, link/unlisted, Share text with state in its
accessible name. Inbox badges are filled violet/black and hide zero; expanded deck
rows use counts, the rail a violet dot. Deck search has clear/empty states and
never changes editor state or stored data.

## Brand and card symbols

`app/BrandMark` is a decorative currentColor SVG: three fanned cards, cut back to
read in one colour at 16 px. The surrounding link/wordmark supplies its name.
Inline size is `--brand-mark-h` (1 cap); standalone rail/phone size is 16 px. The
favicon shares the geometry and adapts to browser chrome. Do not add raster logos.

Mana uses Scryfall static SVG symbols, including combined hybrid/Phyrexian forms,
in fixed boxes with spoken labels. Before load/on failure, retain plain token
coloured discs: black has a pale ring; generic costs retain centered bold numerals.
No layout movement. ColourIdentity uses 12 px pips with a single written accessible
name. Charts pair mana colours with named legends; colour is never the only cue.
Identity accents never tint text/controls or replace semantic state colours.

## Components

`ui/` is generic; `mtg/` presents Magic facts. Keep `/design` variants/states and
this public export inventory synchronized. A new component needs an example.

<!-- inventory:start -->
Everything below is exported from `ui/index.ts`. None of it knows about Magic
or about this app.

| Component | For | Variants |
| --- | --- | --- |
| `Button` | Any action | `default` `ghost` `primary` `accept` `reject` `agent`; `sm` `md` `touch`; `kbd` hint; `reach` (invisible padding to `--tap` tall, for compact Edit controls); disabled is one neutral look for every variant: no fill, `--disabled-ink` label and `--disabled-line` border, visible but clearly unavailable |
| `ButtonLink` | A link drawn as a Button, for an action that navigates | `default` `ghost` `primary`; `sm` `md` `touch` |
| `IconButton` | An action with only an icon; `label` is required | `sm` `md` `touch`; tone `danger` (grey bordered, red only on hover or focus: row Delete and Remove) |
| `Icon` | Named line icons; `ui/Icon.tsx` owns the current name set | `size` |
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
| `Select` | Pick one of a list: a select-only combobox over the same list as MultiSelect, never a native `<select>` | `options` (item `disabled`, `title`); `placeholder`; `invalid`; `disabled`; `defaultOpen` (style guide only) |
| `Checkbox` | A native checkbox, restyled | |
| `MultiSelect` | Searchable native-checkbox group with removable selected chips | controlled `value`, async `onChange`, empty state, `compact` trigger summary, dropdown `footer` |
| `Field` | Label, control and hint, wired together | |
| `Meter` | A bar toward a limit | `neutral` `agent` `warn` `add`; clamps to 0-100% |
| `Box` | A bordered container | tone `neutral` `agent` `warn`; `dashed`; padding `none` `sm` `md` |
| `Eyebrow`, `Section` | The small uppercase label, and a titled group with an `aside` | `Eyebrow as` and `Section heading` preserve semantic h2/h3 hierarchy |
| `Stepper` | A number you can type or nudge, with side-by-side arrows that never read as a dropdown | `onStepUp`, `onStepDown`; `upLabel`, `downLabel`; `upDisabled`, `downDisabled` stay focusable; `disabled` |
| `Stack`, `Row`, `Spacer` | Layout on the spacing scale `--s-1` to `--s-8` | `gap` 1-8; Row `align`, `justify`, `wrap` |
| `Text` | Text with a tone, size and weight from the tokens | tone: 8; size `xs` to `xl`; `mono`; weight 3 |
| `Banner` | A full-width strip under a header | `agent` `warn` `error` `neutral`; `actions` |
| `ErrorNote`, `EmptyState`, `Loading` | An inline error, "nothing here yet", and a loading line | EmptyState `heading` `h1` `h2` when it is all the page has |
| `Dialog` | Native modal: showModal, browser focus containment/return, Esc, backdrop click, `data-autofocus` | width `md` `lg` `wide`; placement `center` `sheet` `drawer`; `footer`; `onEscape` |
| `ConfirmDialog` | A question with Cancel and a confirm button | tone `default` `danger` |
| `Explainer` | Specifically labelled help for unfamiliar concepts | closed, open, keyboard focus, close/Escape and focus return; `defaultOpen` (style guide only); `control` (opened by a menu item, beside its anchor, no trigger of its own) |
| `Popover`, `placeNear` | A panel anchored to an element, kept inside the viewport | placement `below` `right` `above` `left`; `align`; `fitAnchor` |
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

## Overlays and controls

- **Dialog** uses native `showModal()`, top-layer rendering, outside inertness,
  browser focus containment/return and safe `data-autofocus`. Do not restore a
  manual Tab trap, counted body lock or body-overflow mutation. Modal/backdrop CSS
  contains scroll progressively. Keep busy cancellation guards. Escape cancels
  the focused inline editor before closing its enclosing sheet. Dragging selection
  onto the backdrop does not dismiss; an actual backdrop click does.
- **Sheet/drawer** are Dialog placements with 44 px headers and internal scrolling.
  Drawer uses `--drawer-w` at the left; sheet uses `--sheet-max-h`/`--sheet-content-h`
  at the bottom. Only explicit details actions cover the deck list. Tablet review
  keeps Reviewing, remaining count, Current proposal and Done beneath navigation.
- **Popover** uses native top layer, CSS anchors, feature-detected fallbacks and
  ResizeObserver. Portals stay within dialog inertness boundaries. Keep native
  dismissal synchronized with controlled state. Escape dismisses only the active
  overlay; screen hotkeys defer. Labelled triggers toggle. Context menus dismiss
  on a new outside press.
- **Explainer** opens a named region, focuses its explanation, and dismisses on
  Close/Escape/outside focus/click with trigger return. Keep the trigger's Tab place.
- **Menu** uses feature-detected native focusgroup or one legacy ARIA keyboard
  system. Preserve pointer versus keyboard entry, arrows, disabled items,
  typeahead and Tab; never layer two navigation systems.
- **Select** is the shared styled select-only combobox, never a native select.
  Focus stays at its trigger with `aria-activedescendant`; arrows/Home/End/Page
  keys/typeahead move, Enter/Space commit, Escape cancels and Tab commits then
  moves on. Stable trigger width; list no narrower, flips at viewport edges and
  scrolls. Select/MultiSelect/Menu share one rotating Chevron and token insets.
- **MultiSelect** is a named native-checkbox group with search, selected chips and
  Done. Tab reaches each control; Space toggles; Escape/Done return to trigger.
  Busy writes disable choices while preserving focus. Empty/no-match text is not
  an option. Do not restore mixed tabbable buttons inside a listbox.
- **Forms** use native constraints where suitable. Invalid submit remains reachable
  and focuses a persistent error; typing does not repeatedly announce validation.
  Writes freeze controls and prevent accidental dialog dismissal. IME Enter/Escape
  retain composition semantics, including Safari keyCode 229.
- **Toast** remains available while hovered/focused; reconcile pause state when an
  action is removed or the toast changes. Later notifications must expire normally.
  Desktop placement is bottom-left over the sidebar, clear of deck/panel actions;
  narrower placement is bottom-center. Secondary actions wrap below when needed.
- **Motion/focus** retain visible focus and non-spatial feedback with reduced
  motion. No smooth scrolling under reduced motion. Remote refresh never steals
  focus; route changes focus their rendered title once. Native standard scrollbars
  remain usable, including Forced Colors.

## Deck editor and inspection

Toolbar and active filters use the deck canvas with a single bottom hairline.
Add cards gets its own tablet row. `/` focuses it, Enter adds, Shift+Enter shortlists and keeps
results, and Escape returns focus to the input. Fit results within the viewport.
Only current query/scope results are actionable. Loading, errors and empty results
are statuses.

Filters hide nonmatches and empty groups, showing matched/full counts while
retaining full totals and review scope. The query, Clear and Export matches
sit below tablet navigation. Clearing restores scroll, selection and details.

Desktop columns are at least 300 px, tablet columns 250 px, with 12 px gaps.
Add columns as space permits. Editor rows stay compact; shared readers wrap names.
Fixed tabular prices sit beside mana. Inline flex rows stay atomic across columns.

Card groups open the list. Clicking opens card details and marks that row with
a neutral fill.
Right-click, modifier-click, Shift+Space and the card details action toggle bulk
selection on editable deck rows. Once a selection is active, touch taps can add
or remove cards. Selected rows use a blue outline and fill. The details and
review fills take priority when both states apply. Bulk actions appear below the
list without moving the rows and stick to the bottom of the viewport on tablets.
Maybe rows are whole buttons and stay outside bulk selection. Blank space, mana
and price all open details, and Enter or Space activates the row. Phone rows keep
full mana and price while allowing names to truncate. Full linked notes appear
in details. `?` inspects.

The scrolling tab strip shows Review, Brief, Power, Stats, Checks, Memories and
History. The current tab is bright, underlined and marked `aria-current`.
Review's violet count shows what is waiting. Inspection replaces the strip with
Back naming the origin, card title, status and mana. Keep the previous view
mounted, hidden and inert, retaining drafts, scroll and the review cursor.
Switching cards keeps the first origin. In a sheet, Escape leaves inspection
first. Back uses `--h-back`; the body is a named region.

Type, price and source sit beside the picture, capped at `--card-image-max`.
Rules text keeps a full-width measure. Labels use `--card-label-w` and
stack below 380 px. The picture sticks at sheet widths of 680 px and wider.
Read-only cards and tokens share the layout. `?`, mana taps and unhandled
references open a named CardSheet that returns focus on close.

Secondary purposes sit immediately below primary slot; do not repeat them on list
rows. Board offers only Main deck and server-eligible roles while retaining an
invalid current role for repair; hide it with only Main deck. No visible Board
label. Board and intrinsic-width Copies share a wrapping row. Copies is Stepper:
down/up shafted arrows, 44 px touch targets, visible label, direct typing and burst
updates settled into one command/undo. Floor 1; Cut removes. Ceiling/eligibility
come from `copy_limit` and server facts, never Oracle parsing or local restrictions.

History, card decisions, memories and assessments use readable dates or relative times.
Budgets/usage/omissions belong in Settings.
Ownership evidence appears only when present, including basis/attribution. Agent
review status is plain in decisions and unread count belongs in proposal history.
Keep required stale warnings visible without generic revision explanations.

## Secondary workflows

Inbox rules on the selected proposal with optional reason; list review is secondary.
Phone opens the selected proposal below a compact deck heading, then the next on
resolution. Individual decisions precede Review all decisions. Document diffs fold
unchanged lines. Profiles pair their document/Edit with pending edits.

Import uses a wide requested dialog. Source and preview are separate steps;
Formats is a native disclosure. Totals/warnings stay above independently scrolling
changes, destructive cuts first; Apply and optional History note stay in footer.
Nonzero counts focus/reopen their named native-summary section. Empty-destination
previews omit zero counts and start add/maybe sections collapsed; all-collapsed
height follows content. Skipped-line and after-import warnings share one named
callout. Keep errors and retry destination rather than creating duplicate decks.

Settings uses the centered page frame: credentials/setup beside one another on
desktop, then price/memory preferences, data/account controls; stack on tablets.
Method stays visible; Setup instructions starts open until agent contact and
reopens on method/token changes. Explicit disclosure choice survives refresh.
Tokens/apps remain visible. Budget fields precede optional usage with named scopes
and exact values. Secondary text is body-sized; secret labels wrap at row edges.

## Statistics and tokens

Paper overview has a real section heading, large mono deck/family/mana-value/land
figures and family explanation. Values stay next to short labels; source figures
come from the server. Show Land only in the headline: the type grid uses front-face
classification, unlike total land count. Named WUBRG bars distinguish Black from
empty space; “Colours in deck identities” counts Paper decks including each colour.

Deck Stats leads with totals/curve, paired WUBRG donuts and neutral type-group bars.
Fixed cost symbols and recognized mana-source colour-copy counts have separate
units, totals and normalized denominators. Multicolour sources overlap; these are
not available mana, adequacy or casting odds. Show both donuts in one row above
an exact shared colour table, even at minimum panel width; mono is a full ring,
empty explicitly zero. Keep any-colour/flexible/non-coloured/other-face and
incomplete coverage separate in details. Section headings are visually hidden.
Metric and chart labels need no hover.

Scryfall-related Tokens/emblems close the deck list after maybes in its columns.
Each row shows name, P/T/type without redundant Token prefix, real colour pips
and producing-card count. Hover uses its own stored image; ? inspects and click/
Enter opens TokenDetail. Producing cards are normal links. Filters retain tokens
of shown producers with matched/full counts. Distinguish unavailable catalog from
ready/no tokens. Missing images remain absent; no name lookup or inferred quantity.

## Art

`DeckArt`/`DeckThumbnail` consume stored facts without fetching. Admit at most two
partner crops with nonblank artists and HTTPS `cards.scryfall.io/art_crop/` URLs.
No generated/stock/bundled art, constructed API URLs or uncredited crops. Full
inspection pictures retain printed artist/copyright. Decorative images have empty
alt; reserve aspect/space before load/failure and hide credits if the image fails.

| Variant | Placement and frame | Loading |
| --- | --- | --- |
| strip | Roster/public profile/styleguide, 626/457 gallery aspect | Near actual scroll container, 200 px margin |
| editor | Owner header, 200–240 px desktop; 180 px tablet minimum | Eager |
| banner | Phone header, 224 px minimum | Eager |
| showcase | Public page, 176 px; 112 px phone | Eager/high priority |
| backdrop | Sign-in/empty-account panel, one exact-source land per visit | Eager, no rotation |
| feature | Home and Play art with a caption-local scrim | Eager |
| thumbnail | Sidebar/drawer 16 px | Deferred; keep native-size frame |

Gallery captions sit directly on art without a panel edge. Eased multi-stop black
fade begins 104 px above text, at least 58% at title, 64% commanders and 74% metadata.
No masked blur dependency. Use 21 px names, 12 px commanders (14 px phone); owner
metadata 14 px with 16 px pips. Names/partners wrap and rows stretch to fit. Credits
are unboxed bottom-right with a reserved line, one per crop. Owner-directed rest
opacity 0.22 is intentionally exempt from text contrast; hover/focus raises to full.
Touch stays quiet. Showcase/backdrop keep their credit pills; backdrops name card.

Gallery hover/focus brightens art 8% and border; zoom only without reduced motion.
Desktop menus reveal on hover/focus with an independent contrast floor; touch
keeps them visible with 44 px targets. Menu overlays use 64/74/82% black for rest/
hover/pressed and override generic icon hover. Cards are whole links with commander
links/menu above; focus ring stays inside. Commander names underline only on their
own hover/focus, with 2 px dotted offset. Touch uses padding, never an underline.

My decks uses three desktop columns through 1920 px, two tablet, one phone; from
2200 px its frame alone grows from 1440 toward 1940 px at 2560. Statistics share it.
Missing/failed art retains frame and a restrained 18% identity tint, neutral for
colourless. Identity is a 2 px stripe with split gradients for partners/multicolour,
dark Black and neutral colourless. Never tint text/controls. Rail thumbnails are
decorative with credit tooltip and subdued opacity except active/hovered; deck
names and accessible state remain available.

Backdrops show unblurred land above a solid form panel. Editor hero has 36 px title,
lifecycle, commander/identity, estimate/power/counts and aligned actions. Sharp art
enters from right, capped 626 px (760 px pair), with a dim 34 px blurred wash across
canvas; fallback is black with 12% identity tint. Canvas fade ends at 75% crop;
bottom fade starts 55% and reaches 32% black. Text carries local 64% black scrims
with 104 px horizontal/24 px vertical feather, never a full-width narrow-screen wash.
Over pure white art, preserve title 6.15:1 and soft metadata 4.53:1 floors. Gallery
floors are title 4.88:1, commanders 4.53:1 and full-opacity metadata 6.75:1; named
semantic counts retain their measured floors in tokens. Credits keep the explicit
exception. Tokenized fades derive from `--bg`.

Archived notice sits above the title within existing hero space, with local scrim,
no toolbar gap: read-only, switch lifecycle or fork. Fork remains a desktop/tablet
header action and in the compact bar. Phone Back row carries short notice above
title without growing 224 px frame; phone capabilities remain read-and-decide.
Non-archived notices retain their placement. Phone title is 26 px; tablet longer
content may grow header. Add/Find share a tablet row with filters beneath.

Desktop native list scrolling continuously folds hero to 56 px compact title/actions.
Crossfade only accessible active header; preserve mounted list/panel, columns,
scroll range and end space. Panel-only scroll never folds. Find/review reveals
expose workspace before measurement; filter clear restores positions. Never
intercept wheel/trackpad momentum or add timed easing, including reduced motion.
Tablet/phone/public heroes leave naturally with window scroll. Compact phone/
tablet header retains deck name/lifecycle beneath navigation; review/filter strips
stack below. End gutter 16 px; add 64 px bulk clearance only while selection exists.

Art stays out of Inbox, Activity, Memories, Settings, dialogs and solo playtester
content. Home and Play use stored land or commander art. Setup pairs compatibility with launch.
Saved games show deck name, status and update time.

Tables share game-ui chrome, playmat, card frames and zone headings. Compact
seats show life, zone counts and commander links. Solo hands preview three
rules lines. Selection reveals full rules in the rail.
Live inspection returns focus to its trigger.
Preview/live art follows the [client presentation contract](../../../docs/game-client.md#package-privacy-and-presentation-boundaries).
