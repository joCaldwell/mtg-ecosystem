# Phase 4c visual identity QA — 2026-10-26

Built from up-to-date main (merged #10), branch `v3/4c-visual`. The PR is the phase report. These artifacts deliberately live only on `qa/v3-4c-screenshots`.

28 production screenshots and axe 4.14.0 scans at 375, 768, 1024 and 1280 px. All have zero axe violations and no horizontal page overflow. `qa-final.json` records the viewport, images, scroll holder/position, folding state and scans. All pictured accounts are synthetic; card facts come from the online backup's cards table.

## Screenshots

| Width | Home | Deck | Scrolled deck | Navigation | Sign-in | First run | Public |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 375 | [Home](decks-375.png) | [Deck](deck-375.png) | [Scrolled](deck-scrolled-375.png) | [Navigation](sidebar-375.png) | [Sign-in](signin-375.png) | [First run](first-run-375.png) | [Public](public-375.png) |
| 768 | [Home](decks-768.png) | [Deck](deck-768.png) | [Scrolled](deck-scrolled-768.png) | [Navigation](sidebar-768.png) | [Sign-in](signin-768.png) | [First run](first-run-768.png) | [Public](public-768.png) |
| 1024 | [Home](decks-1024.png) | [Deck](deck-1024.png) | [Scrolled](deck-scrolled-1024.png) | [Navigation](sidebar-1024.png) | [Sign-in](signin-1024.png) | [First run](first-run-1024.png) | [Public](public-1024.png) |
| 1280 | [Home](decks-1280.png) | [Deck](deck-1280.png) | [Scrolled](deck-scrolled-1280.png) | [Navigation](sidebar-1280.png) | [Sign-in](signin-1280.png) | [First run](first-run-1280.png) | [Public](public-1280.png) |

At 375 the navigation screenshot is the phone shell: there is no sidebar under D17. At 768 it is the open navigation drawer; 1024/1280 show the rail. Phone first run retains its larger-screen creation guidance. Art does not add editing rights.

## Design skill checklists and changes

The runtime has no Skill tool. Read the four installed `design:*` SKILL.md files and applied their checklists directly; no automated skill invocations or external reviewer runs are claimed.

- **design-system, before and after:** audited tokens, CardImage/CardSheet, public header, home, sidebar, sign-in and first run. Replaced the one-off public-art treatment with DeckArt/DeckThumbnail. Added only tokenized strengths, fades, blur, dimensions and identity opacity. `/design` has all variants, missing-artist/commander and unavailable-image states. DESIGN.md records rules and final audit.
- **ux-copy:** “Art by <artist> · <card>” everywhere, retaining the established public wording. Both artists are named for pairs. Rail thumbnails have the same accessible name and native title.
- **design-critique:** reviewed all four widths and asked “is this distracting?” Short crops frame selection; names/actions lead, editor rows remain text, and the long-list desktop banner folds. Kept backdrops blurred/dark. Replaced squeezed pair credits with separate wrapping lines; retained the phone's read/decide limits and existing tablet drawer. Art is absent from inbox/activity/memories/Settings/dialog/playtest content (the app-wide rail retains its deck thumbnails).
- **accessibility-review:** fixed the sign-in screen's missing main landmark/skip target. All 28 final production scans have zero violations. Art section at `/design` independently passed a scoped scan. Full `/design` still has unrelated pre-existing playtest example roles, heading order, listbox label and duplicate landmark findings; these were not hidden or changed by this phase. NVDA/VoiceOver were unavailable.

## Contrast, loading and interaction evidence

Axe marks some image/gradient-backed text indeterminate; zero automated contrast violations alone is not the proof. `contrast.json` computes every text/semantic foreground against pure white art under each minimum overlay. Quietest text: **6.51:1** at the content fade; **5.81:1** on backdrops. Opaque credits: **12.06:1**. All exceed 4.5:1. The brighter part of a crop has no text over it.

All new images use empty alt, lazy loading, async decoding, stored art_crop URLs and fixed dimensions/aspect with reserved frames. Artist names are visible text, or accessible title/name on thumbnails; unknown artists suppress art. A simulated native image error preserves exact geometry and credit. A blocked-image production run measures identical banner geometry before and after the art loads, with zero art-load shifts. Its initial async data-shell transition also records a 0.1256 shift from the existing footer leaving the loading shell; that is reported separately, not attributed to image loading or hidden. Reduced-motion emulation has `--fast: 0ms` and no running animations. Native keyboard activation reaches the sign-in main via the skip link. See `interactions.json`.

Real wheel input moved the window by 650 px at 375/768/1024. At 1280 it moved the list pane, folded art, and kept the window at zero. Scrolling only the side panel did not fold art or move the list. A short desktop list without enough overflow remains stable rather than folding and clamping itself back to the top. The #9 scroll holder and body contract are unchanged.

T3 preview was used first. Two attempts to focus the sign-in input returned “Preview automation click timed out after 15000ms”; its intersection-observer check also timed out. Final captures and interaction checks therefore use local Chromium/Playwright, native wheel and keyboard input. The production CSP stayed enabled; axe was served only as an ignored local QA asset, then removed. No production authentication bypass was added: the serving harness alone supplies synthetic principals through createApp's test seam.

## Performance and data safety

- Public initial JavaScript: **89,841 gzip bytes**, under 90,000. `budget.json` is the unmodified manifest check; `scripts-final.json` is the actual production browser request set and independently totals the same bytes. The read-only brief uses a display-only component to keep editor state/loader out of the visit.
- Sample phone roster: three unique art-crop JPEGs, **264,091 bytes** total (65,180 / 119,630 / 79,281). No normal/large images, preloads, bundled art, or browser Scryfall API calls.
- Online SQLite backup through a read-only connection into this worktree's `.dev/`. Copy migrated from schema 13 to 15; artist+matching crop refreshed from the existing cached oracle bulk file on the copy only (38,998 records processed; 35,135 credited crops). Integrity `ok`; foreign-key check empty. No new migration, runtime dependency, Railway call or main-checkout data mutation.
- No generated, stock or non-Scryfall decorative image ships. Every shown art has a visible or accessible artist credit. Brand marks remain CSS, not image assets.
