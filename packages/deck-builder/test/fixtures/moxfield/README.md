# Moxfield export fixtures

Real text exports of six public Commander decks on Moxfield, retrieved
2026-10-25 from the deck page's Export dialog (one file per button:
"Copy for Moxfield", "Copy for MTGO", "Copy Plain Text", plus one "Copy for
Arena"). They pin the format `src/deck/interop.ts` documents. Saved byte for
byte (LF, no trailing newline).

| Files | Deck | What it exercises |
| --- | --- | --- |
| `rograkh-silas.*` | https://moxfield.com/decks/79hYZQUBdUaA9xD8zLX4vQ | partner commanders, foils |
| `bombadil-jegantha.*` | https://moxfield.com/decks/akQNNhWe1EO_3gSwpak05g | companion under SIDEBOARD:, Arena headings |
| `stickers-attractions.*` | https://moxfield.com/decks/w7hBUm_lL0qd8dYa9_N3mg | ATTRACTIONS: and STICKERS: boards |
| `precon-hail-caesar.*` | https://moxfield.com/decks/hvwCjVVsAEOBXY5fP57jzQ | a split card written with one slash |
| `najeela.*` | https://moxfield.com/decks/jT8Y9X4tlUmeNZ2AjkD1Vg | sideboard, foils, collector number 1492★ |
| `kinnan-midrange.*` | https://moxfield.com/decks/OYpsy84lZU-HPrQiW9hmdQ | etched (*E*), 4-letter sets, 1F★ |

The focused suite retains two representative exports. Plain-text exports and the
Najeela printing export also support ux-eval; unused export variants were removed.
