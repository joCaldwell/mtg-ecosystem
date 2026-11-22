# Design system

A dark, dense tool for reading card text and making decisions. `/design` is the
live component reference. [ARCHITECTURE](ARCHITECTURE.md) owns package boundaries,
data flow, routes and lifetime; this file owns appearance and interaction.
Exact measurements live in tokens and CSS Modules; regression floors live in
`browser-checks/check-*.mjs`.

## Principles

1. **The deck list anchors editing.** Details use the side panel, banner or hover
   card. Only an explicitly requested dialog/sheet may cover it.
2. **Colour carries meaning.** Neutral black surfaces carry structure; features
   do not acquire their own colours.
3. **Text first in the editor.** Images are on-demand references, not card-row art.
4. **Density supports reading.** Desktop rows are compact; use space between groups,
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

The canvas is true black with neutral near-black surfaces and opaque popovers;
transparent white edges and hovers lift them without blue greys. Fallback stacking
is sticky < popover < dialog < hover < toast; native overlays use the top layer. The palette derives from [T3 Code's redesign dark theme](https://github.com/pingdotgg/t3code/blob/main/apps/web/src/index.css).

Text tiers are `--text`, `--text-soft`, `--muted`, `--faint`; all meaningful text
passes 4.5:1 on its actual surface/tint. Disabled ink is for disabled controls
only. Primary action ink stays black; semantic actions retain their own colours.
Forced Colors preserves outlines and selected/current states.

Sizes use rem with root 100%, so user text preferences scale them. Controls grow
with content and keep minimum targets. ProposalEyebrow uses sentence case for
handles/agent labels. Numbers, timestamps and code use tabular mono.

Headers align h1/actions with content, without grey bands or explanatory
subtitles. Warnings stay by the decision. Prose keeps a reading measure; tables
scroll in their own container.

## Writing in the UI

Use sentence case, outcome-led actions, useful empty states and no exclamation
marks. Say “Accept the edit”, not “OK”. Call owner choices decisions, never
“rules” or “rulings”; preserve user-authored text. Revision numbers belong to agent-facing records.
Use one name per action: “New deck” opens creation. Explain privacy/permissions
where they apply and keep essential/destructive consequences visible.

Help is sparse and labelled for unfamiliar concepts (“Who can edit?”); no generic
Learn more. Agent-sensitive copy uses `useVoice`; first-run voice depends on real agent writes,
while the sidebar reports authenticated contact, including reads. Errors and
factual hints stay neutral. Sample data is labelled, private and attributed to
sample-agent; connecting an agent is optional.

## Responsive layout and navigation

Phones below 720 px read and decide: no direct deck editing, and unsupported
screens explain the larger-screen requirement. Public pages are responsive readers. Nothing makes the page scroll
sideways at 375 px: wrap names/actions/grids and contain table overflow.

At 720–1099 px the editor scrolls the window, navigation uses a drawer and the
requested deck panel uses a sheet; through 1099 px editor controls have touch-size
targets regardless of pointer. At 1100 px+ panes scroll independently. ARCHITECTURE
owns the approved breakpoints. Coarse pointers omit keyboard hints. Touch uses card
inspection instead of hover. Respect 200% text scaling.

Navigation is Home, My decks, Play, Inbox, Memories, Activity, Profile, Settings,
then decks. The collapsed desktop rail keeps accessible names, lifecycle and Inbox
count; failed preference saves retain the prior state. The footer names the latest authenticated request plus confirmed Agent
context, never implies a persistent connection or mistakes reads for writes.

Lifecycle and sharing labels are neutral, with state in accessible names; Paper
is implicit. The lifecycle menu confirms archiving and ends with “Who can edit?”.
Inbox badges are violet and hide zero. Deck search never changes editor state or stored data.

## Brand and card symbols

`app/BrandMark` is a decorative currentColor SVG of three fanned cards that reads
in one colour at small sizes; the surrounding link/wordmark supplies its name. The
favicon shares the geometry. Do not add raster logos.

Mana uses Scryfall static SVG symbols in fixed boxes with spoken labels. Before
load or on failure, keep plain token-coloured discs with no layout movement.
ColourIdentity has one written accessible name. Charts pair mana colours with named
legends; colour is never the only cue. Identity accents never tint text/controls or
replace semantic state colours.

## Components

`ui/` is generic; `mtg/` presents Magic facts. Keep `/design` variants/states and
this public export inventory synchronized; TypeScript owns exact props. A new
component needs an example.

<!-- inventory:start -->
Everything below is exported from `ui/index.ts`. None of it knows about Magic
or about this app.

| Export | Use |
| --- | --- |
| `Button` | Any action; semantic variants; one neutral disabled look |
| `ButtonLink` | Navigation drawn as a Button; same sizes as Button |
| `IconButton` | Icon-only action; `label` required; `danger` reddens on hover |
| `Icon` | Named line icons; `ui/Icon.tsx` owns names |
| `Kbd` | A key cap |
| `Chip` | Small tag or filter; semantic tones; `dashed` when unconfirmed |
| `Badge` | Count pill |
| `Pill` | Toggle filter in a row; `aria-pressed` carries state |
| `Dot` | Status mark; optional `pulse` |
| `Tabs` | Underlined panel switcher; arrow keys; per-tab badge |
| `Segmented` | One of a few views |
| `Menu` | Button opening actions or a single choice |
| `ContextMenu` | Pointer-positioned action menu; arrow navigation, focus restoration |
| `TextField` | One-line input |
| `TextArea` | Multi-line input; `autosize` |
| `Select` | Select-only combobox; never a native `<select>` |
| `Checkbox` | Restyled native checkbox |
| `MultiSelect` | Searchable checkbox group; removable selected chips |
| `Field` | Label, control and hint |
| `Meter` | Bar toward a limit; clamps to 0–100% |
| `Box` | Bordered container; tones, `dashed`, padding |
| `Eyebrow`, `Section` | Small label; titled group; preserve h2/h3 hierarchy |
| `Stepper` | Typed or nudged number; arrows stay focusable when disabled |
| `Stack`, `Row`, `Spacer` | Layout on spacing scale |
| `Text` | Tokenized tone, size, weight |
| `Banner` | Full-width strip under a header; optional actions |
| `ErrorNote`, `EmptyState`, `Loading` | Error, empty and loading lines; EmptyState may own the page heading |
| `Dialog` | Native modal, sheet or drawer; see overlays |
| `ConfirmDialog` | Question with Cancel and confirm; `danger` tone |
| `Explainer` | Labelled help for unfamiliar concepts; can open from a menu item |
| `Popover`, `placeNear` | Anchored panel inside the viewport |
| `ToastProvider`, `useToast` | One toast with optional actions |
| `DiffView` | Added and deleted lines; folds unchanged runs |
| `Table`, `ClickableRow` | Plain tables; rows that open |
| `CounterControl` | Increment/decrement with readout |
| `KeyValue` | Label/value list |
| `ResizeHandle` | Keyboard-accessible resize edge; caller owns value bounds |
| `Page`, `PageHeader`, `SplitPane` | Screen frame; list/detail columns |
| `useHotkeys` | Screen shortcuts; ignored while typing except Escape |
| `pressesFocusedControl` | Whether Enter/Space presses focus; shortcuts stand down |
| `useLocalStorage` | localStorage-backed state |
| `tokenPx` | Size token as a number |
| `useNearView` | Latching near-viewport check; defers downloads |
<!-- inventory:end -->

## Overlays and controls

- **Dialog** uses native `showModal()`, top-layer rendering, outside inertness,
  browser focus containment/return and safe `data-autofocus`. Do not restore a
  manual Tab trap, counted body lock or body-overflow mutation. Keep busy
  cancellation guards. Escape cancels a focused inline editor before closing its
  sheet. Dragging a selection onto the backdrop does not dismiss; a click does.
- **Sheet/drawer** are Dialog placements; only explicit details actions use them
  to cover the deck list.
- **Popover** uses native top layer and CSS anchors with feature-detected fallbacks.
  Portals stay within dialog inertness. Keep native dismissal synchronized with
  controlled state. Escape dismisses only the active overlay; screen hotkeys defer.
- **Explainer** opens a named region, focuses its explanation and returns focus to
  its trigger on Close, Escape or outside interaction.
- **Menu** uses native focusgroup or one legacy ARIA keyboard system, never both.
- **Select** keeps focus on its trigger with `aria-activedescendant`; Enter/Space
  commit, Escape cancels, Tab commits and moves on. Select, MultiSelect and Menu
  share one rotating chevron.
- **MultiSelect** is a named native-checkbox group: Tab reaches each control, Space
  toggles, Escape/Done return to the trigger. Empty/no-match text is not an option.
- **Forms** use native constraints. Invalid submit focuses a persistent error;
  typing does not repeatedly announce validation. Writes freeze controls and
  prevent accidental dismissal. IME Enter/Escape retain composition semantics,
  including Safari keyCode 229.
- **Toast** stays while hovered/focused and later notifications expire normally.
- **Motion/focus** keep visible focus and non-spatial feedback under reduced motion,
  with no smooth scrolling. Remote refresh never steals focus; route changes focus
  their title once. Native scrollbars remain usable, including Forced Colors.

## Deck editor and inspection

Add cards keeps `/`, Enter, Shift+Enter (shortlist, keep results) and Escape.
The results stay within the available viewport and Escape returns to the input.
Loading, errors and empty results are statuses, not listbox options.

Filters show matched/full counts and keep global totals and review scope
unchanged. Clearing restores scroll, selection and open details.

Add columns at wide sizes rather than stretching them. Main deck rows have a
selection checkbox and support Shift+Space. Bulk actions stay by the list header.
Maybe rows are whole buttons and do not join bulk actions requiring main membership;
`?` inspects.

Panel navigation is one line: Review, Brief, Power, Stats, Checks, Memories,
History, with `aria-current` on the current view. The strip scrolls when text
needs more space. Brief drafts survive view changes. Inspection replaces navigation
with a Back control naming the origin. Keep the previous view mounted, hidden and
inert, preserving drafts, scroll and review cursor. In a sheet, Escape returns from
inspection first; explicit Close closes the sheet.

Card facts keep rules text at a constant measure. `?` and inline card references
open CardSheet with focus return.

Secondary purposes sit below the primary slot, not on list rows. Board offers
only Main deck and server-eligible roles, retaining an invalid current role for
repair. Copies is a Stepper with direct typing; bursts settle into one command/undo.
Floor 1; Cut removes. Ceiling and eligibility come from `copy_limit` and server
facts, never Oracle parsing or local restrictions.

Budgets and usage belong in Settings. Keep required stale warnings visible without
generic revision explanations.

## Secondary workflows

Inbox decides the selected proposal with an optional reason; individual decisions
precede Review all decisions. Import previews destructive cuts first and keeps its
retry destination rather than creating duplicate decks. Settings setup
instructions start open until agent contact.

## Statistics and tokens

Statistics figures come from the server. Deck Stats leads with totals/curve, paired WUBRG donuts and neutral type bars.
Fixed cost symbols and mana-source colour counts keep separate units, totals and
denominators; they are not available mana, adequacy or casting odds. Keep
any-colour, flexible, colourless and incomplete coverage separate. Labels need no hover.

Scryfall-related tokens/emblems close the deck list after maybes with
producing-card counts; click/Enter opens TokenDetail. Distinguish an unavailable catalog from no tokens.
Missing images remain absent; no name lookup or inferred quantity.

## Art

`DeckArt`/`DeckThumbnail` consume stored facts without fetching. Admit at most two
partner crops with nonblank artists and HTTPS `cards.scryfall.io/art_crop/` URLs.
No generated/stock/bundled art, constructed API URLs or uncredited crops. Full
inspection pictures retain printed artist/copyright. Decorative images have empty
alt; reserve space before load/failure and hide credits if the image fails.

| Variant | Placement | Loading |
| --- | --- | --- |
| strip | Roster, public profile, styleguide | Near its scroll container |
| editor | Owner header | Eager |
| banner | Phone header | Eager |
| showcase | Public page | Eager/high priority |
| backdrop | Sign-in/empty-account panel, one land per visit | Eager, no rotation |
| feature | Home and Play, caption-local scrim | Eager |
| thumbnail | Sidebar/drawer | Deferred |

Text over art uses local scrims derived from `--bg`, never a full-width wash. One
credit per crop; its owner-directed quiet rest opacity is the only text-contrast
exception. Gallery cards are whole links; zoom only without reduced motion.

Missing or failed art keeps its frame with a restrained identity tint.

The editor hero folds to a compact title with native list scrolling; panel-only
scroll never folds. Never intercept wheel/trackpad momentum or add timed easing.
Archived notices offer switch lifecycle or fork.

Art stays out of Inbox, Activity, Memories, Settings, dialogs and solo playtester
content. Home and Play use stored land or commander art.

Tables share game-ui chrome. Live inspection returns focus to its trigger. Preview/live art follows the
[client presentation contract](../../../docs/game-client.md#package-privacy-and-presentation-boundaries).
