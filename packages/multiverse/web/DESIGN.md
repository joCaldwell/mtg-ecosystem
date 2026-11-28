# Design system

A dark, dense tool for reading card text and making decisions. `/design` is the
live component reference. [ARCHITECTURE](ARCHITECTURE.md) owns boundaries, data
flow, routes, breakpoints and lifetime; this file owns appearance and interaction.
Exact measurements live in tokens and CSS Modules; regression floors live in
`browser-checks/check-*.mjs`.

## Principles

1. **The deck list anchors editing.** Details use the side panel, banner or hover
   card; only an explicitly requested dialog/sheet may cover it.
2. **Colour carries meaning.** Neutral black surfaces carry structure; features
   do not acquire their own colours.
3. **Text first in the editor.** Images are on-demand references, not card-row art.
4. **Density supports reading.** Desktop rows are compact, spacing groups rather
   than rows; touch rules take precedence at narrower widths.
5. **Every agent write is visibly an agent's.** Use violet and the agent's name;
   public readers see generic agent attribution and private names never leak.
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
4.5:1 on its actual surface/tint, disabled ink is for disabled controls only and
primary action ink stays black. Forced Colors preserves outlines and
selected/current states. Sizes use rem at root 100% so user text preferences scale
them; numbers, timestamps and code use tabular mono. Headers align h1/actions with
content, without grey bands or explanatory subtitles. Prose keeps a reading
measure; tables scroll in their own container.

## Writing in the UI

Use sentence case, outcome-led actions (“Accept the edit”, not “OK”), useful empty
states and no exclamation marks. Call owner choices decisions, never “rules” or
“rulings”; preserve user-authored text. Revision numbers belong to agent-facing
records. One name per action (“New deck”). Explain privacy/permissions where they
apply and keep destructive consequences by the decision. Help is sparse and
labelled for unfamiliar concepts (“Who can edit?”), never a generic Learn more.
Agent-sensitive copy uses `useVoice`; errors and factual hints stay neutral. Sample
data is labelled, private and attributed to sample-agent; connecting an agent is
optional.

## Responsive layout and navigation

Phones read and decide (spec §11.2); [ARCHITECTURE](ARCHITECTURE.md#responsive-composition-and-navigation)
owns breakpoints, scrolling and composition. Nothing scrolls sideways at 375 px.
Tablet editor controls have touch-size targets regardless of pointer; coarse
pointers omit keyboard hints and touch uses card inspection instead of hover.
Respect 200% text scaling.

Navigation is Home, My decks, Play, Inbox, Memories, Activity, Profile, Settings,
then decks; Activity is its own page and Profile has no tabs. The desktop toggle
keeps its position when the rail collapses; the rail keeps accessible names,
lifecycle and Inbox count. The footer names the latest authenticated request plus
confirmed agent context, never implying a persistent connection or treating reads
as writes. Lifecycle and sharing labels are neutral, with state in accessible
names; Paper is implicit. The lifecycle menu confirms archiving and ends with “Who
can edit?”. Inbox badges are violet and hide zero. Deck search never changes
editor state or data.

## Brand and card symbols

`app/BrandMark` is a decorative currentColor SVG of three fanned cards, legible in
one colour at small sizes and shared by the favicon; no raster logos. Mana uses
Scryfall static SVG symbols in fixed boxes with spoken labels; token-coloured discs
hold layout before load or on failure. ColourIdentity has one written accessible
name; charts pair mana colours with named legends. Identity accents never tint
text/controls or replace semantic state colours.

## Components

`ui/` is generic; `mtg/` presents Magic facts. A new component needs a `/design`
example; `test/web-architecture.test.ts` checks this inventory against
`ui/index.ts`, and TypeScript owns props.

<!-- inventory:start -->
Exported from `ui/index.ts`; nothing here knows about Magic or this app.

| Exports | Use |
| --- | --- |
| `Button`, `ButtonLink`, `IconButton`, `Icon`, `Kbd` | Actions with semantic variants, one neutral disabled look; IconButton requires `label` |
| `Chip`, `Badge`, `Pill`, `Dot` | Tags/filters (`dashed` when unconfirmed), counts, `aria-pressed` toggles, status marks |
| `Tabs`, `Segmented`, `Menu`, `ContextMenu` | Panels, few-view choice, action menus restoring focus |
| `TextField`, `TextArea`, `Select`, `Checkbox`, `MultiSelect`, `Field`, `Stepper` | Inputs; disabled Stepper arrows stay focusable |
| `Meter`, `Box`, `Eyebrow`, `Section`, `Stack`, `Row`, `Spacer`, `Text` | Layout and tokenized text; Section keeps h2/h3 hierarchy |
| `Banner`, `ErrorNote`, `EmptyState`, `Loading` | Strips and status lines; EmptyState may own the heading |
| `Dialog`, `ConfirmDialog`, `Explainer`, `Popover`, `placeNear`, `ToastProvider`, `useToast` | Overlays (below) |
| `DiffView`, `Table`, `ClickableRow`, `CounterControl`, `KeyValue`, `ResizeHandle` | Diffs, tables, counters, label/value lists, keyboard resize |
| `Page`, `PageHeader`, `SplitPane` | Screen frame and list/detail columns |
| `useHotkeys`, `pressesFocusedControl`, `useLocalStorage`, `tokenPx`, `useNearView` | Shortcuts (ignored while typing except Escape), storage, sizes, deferred loads |
<!-- inventory:end -->

## Overlays and controls

- **Dialog** uses native `showModal()` (top layer, outside inertness, browser focus
  containment/return, safe `data-autofocus`), never a manual Tab trap or
  body-scroll lock, and keeps busy cancellation guards. Escape cancels a focused
  inline editor before closing its sheet; a selection dragged onto the backdrop
  does not dismiss. Sheet/drawer are Dialog placements.
- **Popover** uses the top layer and CSS anchors with feature-detected fallbacks,
  stays within dialog inertness and keeps native dismissal synchronized. Escape
  dismisses only the active overlay; screen hotkeys defer. **Explainer** opens a
  named region and returns focus to its trigger.
- **Menu** uses native focusgroup or one legacy ARIA keyboard system, never both.
  **Select** keeps focus on its trigger with `aria-activedescendant`; Enter/Space
  commit, Escape cancels, Tab commits and moves on. **MultiSelect** is a named
  native-checkbox group: Tab reaches each control, Space toggles, Escape/Done
  return to the trigger, and empty/no-match text is not an option. All three share
  one rotating chevron; no native selects.
- **Forms** use native constraints: invalid submit focuses a persistent error,
  typing does not re-announce validation, writes freeze controls and prevent
  accidental dismissal, and IME Enter/Escape keep composition semantics (including
  Safari keyCode 229). **Toast** stays while hovered/focused.
- **Motion/focus** keep visible focus and non-spatial feedback under reduced motion,
  with no smooth scrolling; remote refresh never steals focus, and native
  scrollbars stay usable, including Forced Colors.

## Deck editor and inspection

Add cards keeps `/`, Enter, Shift+Enter (shortlist, keep results) and Escape (back
to input); results stay within the viewport, and loading/errors/empty are statuses,
not options. Filters show matched/full counts; clearing restores scroll, selection
and open details. Add columns at wide sizes rather than stretching them.

Clicking a row opens card details with a neutral fill. Right-click, modifier-click,
Shift+Space and the details action toggle bulk selection on editable rows (touch
taps once a selection is active); selected rows use a blue outline and fill, with
details/review fills taking priority. Bulk actions sit below the list without
moving rows and stick to the viewport bottom on tablets. Maybe rows are whole
buttons outside bulk selection. `?` and inline card references open CardSheet with
focus return; rules text keeps a constant measure.

Panel navigation is one scrolling line (Review, Brief, Power, Stats, Checks,
Memories, History) with `aria-current`. Inspection replaces it with a Back control
naming the origin while the previous view stays mounted, hidden and inert, keeping
drafts, scroll and review cursor; in a sheet, Escape leaves inspection first.

Secondary purposes sit below the primary slot, not on list rows. Board offers only
Main deck and server-eligible roles, keeping an invalid current role for repair.
Copies is a Stepper with direct typing whose bursts settle into one command/undo:
floor 1, Cut removes, ceiling from `copy_limit`. Required stale warnings stay
visible without generic revision explanations; budgets and usage belong in
Settings. Inbox decides the selected proposal with an optional reason, individual
decisions preceding Review all decisions. Import previews destructive cuts first.
Settings setup instructions start open until agent contact.

## Statistics and tokens

Figures come from the server (spec §§4.4–4.5, 11.4). Deck Stats leads with
totals/curve, paired WUBRG donuts and neutral type bars, with visually hidden
section headings and visible labels needing no hover. Fixed cost symbols and
mana-source counts keep separate units and denominators; any-colour, flexible,
colourless and incomplete coverage stay separate. Costs and deltas show
source/date and unpriced counts. Tokens/emblems close the deck list after maybes
with producing-card counts; click/Enter opens TokenDetail. Distinguish an
unavailable catalog from no tokens; missing images stay absent.

## Art

Real card art only (spec §11.4). `DeckArt`/`DeckThumbnail` consume stored facts
without fetching and admit at most two partner crops with nonblank artists and
HTTPS `cards.scryfall.io/art_crop/` URLs; backend `src/art.ts` admits and
`domain/art.ts` repeats the check. No generated/stock/bundled art, constructed API
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
exception. Gallery cards are whole links; zoom only without reduced motion.
Missing or failed art keeps its frame with a restrained identity tint. The editor
hero folds to a compact title without timed easing
([header scroll](ARCHITECTURE.md#deck-workflows)); archived notices offer switch
lifecycle or fork.

Art stays out of Inbox, Activity, Memories, Settings, dialogs and solo playtester
content. Tables share game-ui chrome; preview/live art follows the
[client presentation contract](../../../docs/game-client.md#package-privacy-and-presentation-boundaries).
