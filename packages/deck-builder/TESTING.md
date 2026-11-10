# Focused test suite

The test gate is intentionally small for rapid design iteration. Run `npm run check`.
Keep capability permissions and tenant isolation, viewer public-state privacy,
migration content preservation, backup/restore smoke, exact card resolution and
suggestions, membership/undo, proposal acceptance/rejection, two import examples
per format, hosted auth/session and CSP headers, one real MCP client smoke, and
the web architecture guard. Avoid UI copy/markup/CSS snapshots and duplicated
transport matrices. These checks are representative, not exhaustive security coverage.

Measured on Node 26.12.1 in this worktree (one full successful run each):

| | Before | After |
| --- | ---: | ---: |
| Tests | 1,320 | 109 |
| Full npm run check | 36.92 s | 6.56 s |
| Test runner | 31.29 s | 1.56 s |

No tests are skipped. Application behavior and browser-checks are unchanged.
Plain-text Moxfield fixtures and the Najeela printing export remain because
browser-checks uses them. The architecture documentation retains its symbology fixture.

## Files deleted outright

These deletions can conflict with concurrent UX work; keep the deletion when
merging test-only updates to these files.

- `test/art.test.ts`
- `test/backup-retention.test.ts`
- `test/client-command.test.ts`
- `test/crossdeck.test.ts`
- `test/deck.test.ts`
- `test/documents.test.ts`
- `test/fixtures/mana-analysis.ts`
- `test/fixtures/moxfield/bombadil-jegantha.copy-for-arena.txt`
- `test/fixtures/moxfield/bombadil-jegantha.copy-for-moxfield.txt`
- `test/fixtures/moxfield/bombadil-jegantha.copy-for-mtgo.txt`
- `test/fixtures/moxfield/kinnan-midrange.copy-for-moxfield.txt`
- `test/fixtures/moxfield/kinnan-midrange.copy-for-mtgo.txt`
- `test/fixtures/moxfield/najeela.copy-for-mtgo.txt`
- `test/fixtures/moxfield/precon-hail-caesar.copy-for-moxfield.txt`
- `test/fixtures/moxfield/precon-hail-caesar.copy-for-mtgo.txt`
- `test/fixtures/moxfield/rograkh-silas.copy-for-moxfield.txt`
- `test/fixtures/moxfield/rograkh-silas.copy-for-mtgo.txt`
- `test/fixtures/moxfield/stickers-attractions.copy-for-moxfield.txt`
- `test/fixtures/moxfield/stickers-attractions.copy-for-mtgo.txt`
- `test/hosted-review.test.ts`
- `test/insert-attribution.test.ts`
- `test/job-diagnostics.test.ts`
- `test/json-array.test.ts`
- `test/lifecycle.test.ts`
- `test/local-logging.test.ts`
- `test/logging.test.ts`
- `test/mana.test.ts`
- `test/memory.test.ts`
- `test/playtest-http.test.ts`
- `test/playtest.test.ts`
- `test/power.test.ts`
- `test/prices.test.ts`
- `test/process-lock.test.ts`
- `test/proposal-cleanup.test.ts`
- `test/runtime-layout.test.ts`
- `test/scryfall.test.ts`
- `test/search.test.ts`
- `test/secondary-slots.test.ts`
- `test/sharing-http.test.ts`
- `test/sourcing-fixtures.ts`
- `test/sourcing.test.ts`
- `test/tags.test.ts`
- `test/tokens.test.ts`
- `test/tomaybe.test.ts`
- `test/tools.test.ts`
- `test/ux.test.ts`
- `test/web/activity.test.ts`
- `test/web/api.test.ts`
- `test/web/art.test.ts`
- `test/web/cards.test.ts`
- `test/web/decision-controls.test.ts`
- `test/web/deck-editor.test.ts`
- `test/web/deckSearch.test.ts`
- `test/web/deckStats.test.ts`
- `test/web/diff.test.ts`
- `test/web/documents.test.ts`
- `test/web/familyOrder.test.ts`
- `test/web/fixtures.ts`
- `test/web/grouping.test.ts`
- `test/web/import.test.ts`
- `test/web/inbox.test.ts`
- `test/web/mana-symbols.test.ts`
- `test/web/marks.test.ts`
- `test/web/maybe-notes.test.ts`
- `test/web/power.test.ts`
- `test/web/price.test.ts`
- `test/web/random.test.ts`
- `test/web/read-only-deck.test.ts`
- `test/web/readOnlyDeck.test.ts`
- `test/web/review.test.ts`
- `test/web/routes.test.ts`
- `test/web/search.test.ts`
- `test/web/secondary-pages.test.ts`
- `test/web/shell.test.ts`
- `test/web/staleChanges.test.ts`
- `test/web/table-components.test.ts`
- `test/web/table.test.ts`
- `test/web/time.test.ts`
