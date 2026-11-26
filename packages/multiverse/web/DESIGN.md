# Design system

A dark, dense tool for reading card text and making decisions. `/design` is the
live component reference. [ARCHITECTURE](ARCHITECTURE.md) owns boundaries, data
flow, routes, breakpoints and lifetime; this file owns appearance and interaction.
Exact measurements live in tokens and CSS Modules; regression floors live in
`browser-checks/check-*.mjs`.

## Principles

1. **The deck list anchors editing.** Details use the side panel, banner or hover
   card. Only an explicitly requested dialog/sheet may cover it.
2. **Colour carries meaning.** Neutral black surfaces carry structure; features
   do not acquire their own colours.
3. **Text first in the editor.** Images are on-demand references, not card-row art.
4. **Density supports reading.** Desktop rows are compact, with space between groups
   rather than rows; touch rules take precedence at narrower widths.
5. **Every agent write is visibly an agent's.** Use violet and the agent's name.
   Public readers see generic agent attribution; private names never leak.
6. **Reasons are cheap.** One optional field accepts Enter to commit a decision.
7. **Numbers are monospaced and supplied by the server.**

## Tokens and type

`src/design/tokens.css` owns colours, spacing, sizes, radii, widths, elevation,
stacking and motion; component CSS uses tokens. `design/base.css` owns the global
`.visually-hidden` utility.

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
transparent white edges and hovers lift them, never blue greys. The palette derives
from [T3 Code's redesign dark theme](https://github.com/pingdotgg/t3code/blob/main/apps/web/src/index.css).
Fallback stacking is sticky < popover < dialog < hover < toast; native overlays use
the top layer.

Text tiers are `--text`, `--text-soft`, `--muted`, `--faint`; meaningful text passes
4.5:1 on its actual surface/tint; disabled ink is for disabled controls only.
Primary action ink stays black. Forced Colors
preserves outlines and selected/current states. Sizes use rem at root 100% so user
text preferences scale them. Numbers, timestamps and code use tabular mono.
Headers align h1/actions with content, without grey bands or explanatory subtitles.
Prose keeps a reading measure; tables scroll in their own container.

## Writing in the UI

Use sentence case, outcome-led actions (“Accept the edit”, not “OK”), useful empty
states and no exclamation marks. Call owner choices decisions, never “rules” or
“rulings”; preserve user-authored text. Revision numbers belong to agent-facing
records. One name per action (“New deck”). Explain privacy/permissions where they
apply and keep destructive consequences and warnings by the decision.

Help is sparse and labelled for unfamiliar concepts (“Who can edit?”), never a
generic Learn more. Agent-sensitive copy uses `useVoice`. Errors and factual hints
stay neutral. Sample data is labelled, private and attributed to sample-agent;
connecting an agent is optional.

## Responsive layout and navigation

Phones below 720 px read and decide: no direct deck editing, and unsupported screens
explain the larger-screen requirement. Public pages are responsive readers. Nothing
scrolls sideways at 375 px. At 720–1099 px the editor scrolls the window, navigation
is a drawer, the deck panel is a sheet and editor controls have touch-size targets
regardless of pointer. At 1100 px+ panes scroll independently. Coarse pointers omit
keyboard hints; touch uses card inspection instead of hover. Respect 200% text scaling.

Navigation is Home, My decks, Play, Inbox, Memories, Activity, Profile, Settings,
then decks; Activity is its own page and Profile has no tabs. The desktop toggle
keeps its position when the rail collapses. The rail keeps accessible names,
lifecycle and Inbox count. The footer names the latest authenticated request plus
confirmed agent context, never implying a persistent connection or treating reads
as writes.

Lifecycle and sharing labels are neutral, with state in accessible names; Paper is
implicit. The lifecycle menu confirms archiving and ends with “Who can edit?”.
Inbox badges are violet and hide zero. Deck search never changes editor state or data.

## Brand and card symbols

`app/BrandMark` is a decorative currentColor SVG of three fanned cards, legible in
one colour at small sizes; the favicon shares it. No raster logos.

Mana uses Scryfall static SVG symbols in fixed boxes with spoken labels; before load
or on failure, token-coloured discs keep layout stable. ColourIdentity has one
written accessible name. Charts pair mana colours with named legends. Identity
accents never tint text/controls or replace semantic state colours.

## Components

`ui/` is generic; `mtg/` presents Magic facts. A new component needs a `/design`
example. `test/web-architecture.test.ts` checks this inventory against
`ui/index.ts`; TypeScript owns props.

<!-- inventory:start -->
Exported from `ui/index.ts`; nothing here knows about Magic or this app.

| Exports | Use |
| --- | --- |
| `Button`, `ButtonLink`, `IconButton`, `Icon`, `Kbd` | Actions with semantic variants and one neutral disabled look; IconButton requires `label` |
| `Chip`, `Badge`, `Pill`, `Dot` | Tags/filters (`dashed` when unconfirmed), counts, `aria-pressed` toggles, status marks |
| `Tabs`, `Segmented`, `Menu`, `ContextMenu` | Panel switching, few-view choice, action menus with focus restoration |
| `TextField`, `TextArea`, `Select`, `Checkbox`, `MultiSelect`, `Field`, `Stepper` | Inputs; Select is never a native `<select>`; Stepper arrows stay focusable when disabled |
| `Meter`, `Box`, `Eyebrow`, `Section`, `Stack`, `Row`, `Spacer`, `Text` | Layout and tokenized text; Section preserves h2/h3 hierarchy |
| `Banner`, `ErrorNote`, `EmptyState`, `Loading` | Strips and status lines; EmptyState may own the page heading |
| `Dialog`, `ConfirmDialog`, `Explainer`, `Popover`, `placeNear`, `ToastProvider`, `useToast` | Overlays; see below |
| `DiffView`, `Table`, `ClickableRow`, `CounterControl`, `KeyValue`, `ResizeHandle` | Diffs folding unchanged runs, tables, counters, label/value lists, keyboard resize |
| `Page`, `PageHeader`, `SplitPane` | Screen frame and list/detail columns |
| `useHotkeys`, `pressesFocusedControl`, `useLocalStorage`, `tokenPx`, `useNearView` | Shortcuts (ignored while typing except Escape), storage, token sizes, deferred downloads |
<!-- inventory:end -->

## Overlays and controls

- **Dialog** uses native `showModal()`, top layer, outside inertness, browser focus
  containment/return and safe `data-autofocus`; never restore a manual Tab trap or
  body-scroll lock. Keep busy cancellation guards. Escape cancels a focused inline
  editor before closing its sheet; dragging a selection onto the backdrop does not
  dismiss. Sheet/drawer are Dialog placements.
- **Popover** uses native top layer and CSS anchors with feature-detected fallbacks;
  portals stay within dialog inertness and native dismissal stays synchronized.
  Escape dismisses only the active overlay; screen hotkeys defer.
- **Explainer** opens a named region and returns focus to its trigger.
- **Menu** uses native focusgroup or one legacy ARIA keyboard system, never both.
- **Select** keeps focus on its trigger with `aria-activedescendant`; Enter/Space
  commit, Escape cancels, Tab commits and moves on. Select, MultiSelect and Menu
  share one rotating chevron; no native selects.
- **MultiSelect** is a named native-checkbox group: Tab reaches each control, Space
  toggles, Escape/Done return to the trigger. Empty/no-match text is not an option.
- **Forms** use native constraints. Invalid submit focuses a persistent error;
  typing does not re-announce validation. Writes freeze controls and prevent
  accidental dismissal. IME Enter/Escape keep composition semantics, including
  Safari keyCode 229.
- **Toast** stays while hovered/focused.
- **Motion/focus** keep visible focus and non-spatial feedback under reduced motion,
  with no smooth scrolling. Remote refresh never steals focus. Native scrollbars
  remain usable, including Forced Colors.

## Deck editor and inspection

Add cards keeps `/`, Enter, Shift+Enter (shortlist, keep results) and Escape (back
to input); results stay within the viewport, and loading/errors/empty are statuses,
not options. Filters show matched/full counts without changing global totals or
review scope; clearing restores scroll, selection and open details.

Add columns at wide sizes rather than stretching them. Clicking a row opens card
details with a neutral fill. Right-click, modifier-click, Shift+Space and the details
action toggle bulk selection on editable rows (touch taps once a selection is
active); selected rows use a blue outline and fill, with details/review fills taking
priority. Bulk actions sit below the list without moving rows and stick to the
viewport bottom on tablets. Maybe rows are whole buttons outside bulk selection.
`?` and inline card references open CardSheet with focus return; rules text keeps
a constant measure.

Panel navigation is one scrolling line: Review, Brief, Power, Stats, Checks,
Memories, History, with `aria-current`. Inspection replaces it with a Back control
naming the origin while the previous view stays mounted, hidden and inert, keeping
drafts, scroll and review cursor. In a sheet, Escape leaves inspection first.

Secondary purposes sit below the primary slot, not on list rows. Board offers only
Main deck and server-eligible roles, keeping an invalid current role for repair.
Copies is a Stepper with direct typing whose bursts settle into one command/undo;
floor 1, Cut removes, ceiling from `copy_limit`. Required stale warnings stay
visible without generic revision explanations. Budgets and usage belong in Settings.

Inbox decides the selected proposal with an optional reason; individual decisions
precede Review all decisions. Import previews destructive cuts first. Settings
setup instructions start open until agent contact.

## Statistics and tokens

Figures come from the server. Deck Stats leads with totals/curve, paired WUBRG
donuts and neutral type bars, with visually hidden section headings and visible
labels needing no hover. Fixed cost symbols and mana-source counts keep separate
units and denominators; they are not available mana, adequacy or casting odds. Keep
any-colour, flexible, colourless and incomplete coverage separate.

Scryfall tokens/emblems close the deck list after maybes with producing-card counts;
click/Enter opens TokenDetail. Distinguish an unavailable catalog from no tokens.
Missing images stay absent; no name lookup or inferred quantity.

## Art

Real card art only. `DeckArt`/`DeckThumbnail` consume stored facts without fetching
and admit at most two partner crops with nonblank artists and HTTPS
`cards.scryfall.io/art_crop/` URLs. No generated/stock/bundled art, constructed API
URLs or uncredited crops. Full inspection pictures keep printed artist/copyright.
Decorative images have empty alt; reserve space and hide credits if the image fails.

| Variant | Placement | Loading |
| --- | --- | --- |
| strip | Roster, public profile, styleguide | Near its scroll container |
| editor | Owner header | Eager |
| banner | Phone header | Eager |
| showcase | Public page | Eager/high priority |
| backdrop | Sign-in/empty-account panel, one land per visit | Eager, no rotation |
| feature | Home and Play (stored land or commander art), caption-local scrim | Eager |
| thumbnail | Sidebar/drawer | Deferred |

Text over art uses local scrims derived from `--bg`, never a full-width wash. One
quiet credit per crop; its owner-directed rest opacity is the only text-contrast
exception. Gallery cards are whole links; zoom only without reduced motion. Missing
or failed art keeps its frame with a restrained identity tint.

The editor hero folds to a compact title with native list scrolling; panel-only
scroll never folds it. Never intercept wheel/trackpad momentum or add timed easing.
Archived notices offer switch lifecycle or fork.

Art stays out of Inbox, Activity, Memories, Settings, dialogs and solo playtester
content. Tables share game-ui chrome; preview/live art follows the
[client presentation contract](../../../docs/game-client.md#package-privacy-and-presentation-boundaries).
