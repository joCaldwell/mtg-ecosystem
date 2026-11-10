# Phase 4b validation

2026-10-25, America/Chicago. Branch `v3/4b-ux`, based on main's merged
phone-scroll fix `268675e`. No hosted service, Railway configuration, or
production data was changed.

## Data and migration

Used SQLite's online backup API with a read-only connection to
`/home/josh/Code/mtg-ecosystem/packages/deck-builder/data/deck-builder.db`.
The snapshot and all QA databases live in this worktree's ignored `.dev/`.
The source file was at schema 10 (corrected in review; an earlier draft said
13); the copy ran migrations 11 through 15.

| Lifecycle | Before | After migration, before seeding |
| --- | ---: | ---: |
| `legacy` | 3 | 0 |
| `archived` | 0 | 3 |
| `paper` | 11 | 11 |
| `theorycrafted` | 4 | 4 |
| All decks | 18 | 18 |

`PRAGMA foreign_key_check` returned no rows; `integrity_check` returned `ok`.
The sample resolved every card in that snapshot: zero missing names. Its
new tags illustrate status/sourcing, and it creates no physical ownership
evidence. Checked the seeded trigger wording against the installed oracle
text: Atraxa proliferates at your end step, not at every player’s end step.
The separate v14→v15 regression checks two tenants, sharing and lineage
columns, relations, all memory/log rows, user prose containing the old words,
reopening, the new CHECK, and an AUTOINCREMENT high-water mark above deleted
ids. The fixture initially made `ci_mask` non-nullable; correcting it to v14's
actual nullable shape fixed the fixture without weakening any assertion.

## Gate and agent interface

- `npm run check` in `packages/deck-builder`: **1,220 tests**, including the
  permission matrix, TypeScript for server/web, migration regressions and web
  architecture rules.
- `tools.test.ts` was updated deliberately: exact new tool names, descriptions,
  rendered decisions, the read/mark loop, and explicit refusal of both old
  names. Its 35 tests pass; no assertion was loosened.
- Read an actual scripted transcript over the official MCP HTTP client on
  port 8795: tool discovery, `start_session`, `get_deck`, `list_decisions`,
  `mark_decisions_reviewed`, an empty queue after marking, and an Archived
  deck's read-only rendering. The new names are present and the old names are
  absent. This verifies the interface; it does not measure a model's deck
  assessment quality. The sanitized transcript is on the QA branch.

## D9 interface changes

D9 renames land in 4b without a deprecation period (v3 plan D19 applies from
public sign-up). Every agent-facing rename:

| Surface | Before | After |
| --- | --- | --- |
| Tool name | `list_rulings` | `list_decisions` |
| Tool name | `mark_rulings_reviewed` | `mark_decisions_reviewed` |
| `remember` argument | `source_ruling` | `source_decision` |
| `get_deck` `sections` value | `rulings` | `decisions` |
| `get_deck` section heading | `## Recent rulings and changes`, `## Awaiting the owner's ruling` | `## Recent decisions and changes`, `## Awaiting the owner's decision` |
| `start_session` heading | `# Rulings you have not read` | `# Decisions you have not read` |
| Card context line (`get_cards`, `search_cards`, `propose_changes`) | `- ruling #N …` | `- decision #N …` |
| Roster and deck header | `legacy`, `Legacy: read-only`, `awaiting ruling` | `archived`, `Archived: read-only`, `awaiting decision` |
| Playbook and instructions | `## Rulings`, Legacy | `## Decisions`, Archived |
| Activity action code (new rows) | `mark_rulings_reviewed` | `mark_decisions_reviewed` |

UI wire only (not MCP): deck `category` value `legacy` → `archived`; card
history `rulings` → `decisions`; log entries' and card decisions'
`ruling_reason` → `decision_reason`. The `log` table and its `ruling_reason`
column keep their names (D9).

Every tool's arguments are strict, so an old argument name fails instead of
being dropped: `remember({…, source_ruling: 1})` returns `'source_ruling' was
renamed to 'source_decision'`, `sections: ["rulings"]` returns `'rulings' was
renamed to 'decisions'`, and any other unknown key returns `'<key>' is not an
argument of <tool>`. The protocol's `_meta` is outside `arguments`. Nested
item objects keep their existing shapes.

## Design checks applied by hand

This Codex runtime has no Claude Code Skill tool. The four requested plugin
SKILL.md files were available locally and were read directly. Their checklists
were applied manually, with the findings below. No automated skill invocation
or independent reviewer is claimed.

| Checklist | Evidence and changes |
| --- | --- |
| `design:design-system` | Audited tokens.css, ui/ and /design before extending them. Reused the touch token, Dialog, CardSheet and read-only composition. Added controlled Advanced, inline metadata, explanatory popovers, drawer placement and the tablet sheet. Documented default/open/saving/empty/selected states, keyboard behavior and focus return in DESIGN.md and /design. |
| `design:ux-copy` | Applied DESIGN.md's sentence case, outcome-led actions and useful-empty-state rules. Audited both deck-first and agent-connected voices, first-run choices and six explainers. Neutral factual hints/errors are shared. Archived/decisions avoids MTG-format/card-ruling ambiguity. The import names Archidekt and Moxfield. No stored prose was rewritten. |
| `design:accessibility-review` | Ran the WCAG 2.1 AA checklist with axe, keyboard, touch, focus, reduced-motion and narrow reflow checks. Fixed faint text, filter opacity, listbox semantics (including an empty disabled option), plain archived titles, skip/main naming, nested Escape, popovers inside their dialog's focus tree, and hidden Advanced controls in focus traps. Named previously unnamed /design examples. |
| `design:design-critique` | Inspected the first-run, deck, drawer and sheet PNGs at 768, 1024, 1280 and 375 px. Kept desktop text density. Flattened the header's Advanced metadata; wrapped tablet toolbar controls; made the retained tablet rail fill its height; aligned the proposal explainer with the list; repaired the first-run Import handoff to select Import and consume its query. Re-captured the affected views. |

`--faint` is #9aa1ab. Its contrast is 7.41:1 on the page, 6.96:1 on surface,
6.49:1 on raised, 6.06:1 on hover, and 6.21:1 on overlay: all exceed 4.5:1.
Focus remains the existing blue ring; attribution remains violet and visible.

## Browser evidence

Chromium, Playwright, axe-core 4.14.0. API harness called `createApp` **without
jobs**, at `DECKBUILDER_PORT=8795`; Vite used `DECKBUILDER_WEB_PORT=5185`.
T3 preview was attempted first; two snapshots failed with
“Preview automation snapshot timed out after 15000ms.” Local Chromium supplied
the screenshots and interaction checks after that failure.

- **27 axe scans, zero violations:** decks, deck, inbox and public deck at
  768/1024/1280/375 px (16); drawer, sheet+explainer, keyboard card dialog,
  selected MultiSelect, Memories, empty listbox and /design drawer (7); an
  entirely empty account at all four widths (4). This includes zero contrast
  and role violations on every required surface.
- Verified keyboard focus return from card inspection and drawer; 16 Tab
  cycles stayed in a dialog containing a hidden Advanced control. Escape
  closes a nested explainer and leaves its sheet open; a second Escape
  closes the sheet. `?` on a focused row opens inspection. A real touch tap
  on a mana cost opens inspection at 768 px.
- Advanced saves per account, stays consistent between deck and Memories,
  and remains closed after reload. Existing accounts keep it open by
  migration; new accounts start closed. Stale-approval warnings and authors
  remain visible regardless of the preference.
- Reduced motion computes `--fast: 0ms`. Archived titles contain no button.
  Pages never overflow horizontally at the requested widths. Boundary checks
  at 719/720, 959/960 and 1099/1100 preserve the phone/editor and shell splits.
- Accessibility semantics were inspected through DOM/roles and focus behavior.
  NVDA/VoiceOver were not available; no real screen-reader session is claimed.

### Scrolling, with real wheel input

No QA check set scrollY or called scrollTo. Short landing/inbox pages need no
scroll when all their content fits; the long sample list proves each viewport's
scroll contract. At every screenshot width a wheel moved the sample's main
content:

| Width | Sample deck | Result |
| ---: | --- | --- |
| 768 | Tablet/window | window 0→550 px |
| 1024 | Tablet/window, rail retained | window 0→550 px |
| 1280 | Desktop/list pane, body pinned | list pane moved; window stayed 0 |
| 375 | Inherited phone/window | window 0→550 px |

Shared pages also moved by wheel on every width. Tables retain their own
horizontal overflow. `TabletShell`, `PhoneShell` and `PublicShell` each call
`useWindowScroll()`; the architecture test protects all three.

### New-account walkthrough

An empty account showed Create, Import and Try the sample at 720+ px, without
agent presence. Clicked Try the sample, opened its private 76-card list, then
opened the lifecycle explainer. The example agent writes were attributed to
`sample-agent`; repeat creation returns that same sample without replacing it.
At 375 px the existing phone view directs initial creation to a larger screen.

A second empty account used Import: named its destination, arrived directly
in the Import tab, pasted the sample's real app-generated export, reviewed the
diff and approved it. The resulting count was 76. The query was consumed and
reloading did not reopen the import dialog.

## Public JavaScript budget

`vite build` plus `node scripts/check-public-bundle.mjs` sums unique startup
JS chunks from the manifest, including React and the entry, using gzip's
default compression. An independent production-browser network capture on
8795 checks the same loaded JS files. It loads no SignedIn chunk.

Final byte count is quoted in the PR and in `phase-4b/bundle-final.json` on the
QA branch. It is below **90,000 bytes**. CSS and interaction-only chunks are not
part of that JS number. Card inspection, public action dialogs and brief editing
load on demand; playtest, settings and /design are lazy routes. No new runtime
or development dependency was added to the repository.

## Evidence and handoff

Screenshots, the sanitized MCP transcript, scan results and browser records
live only on
[qa/v3-4b-screenshots](https://github.com/joCaldwell/mtg-ecosystem/tree/qa/v3-4b-screenshots/phase-4b).
No database or screenshot is committed to main.

Phase 5 inherits the same capability and permission gates, shared-page
projection, lazy public actions, 720/960/1100 boundaries, persisted Advanced,
agent-optional copy hook, and 100-item keyset pages (`before_id`, with
`before_revised_at` for Memories) with `has_more`. Activity's agent filter is
applied by the server before paging and offers every label of the principal;
Memories filters before paging.
Keep the public budget check when adding the suggestion composer. Suggestions
from people, their tone, abuse controls and notifications remain phase 5 work.
