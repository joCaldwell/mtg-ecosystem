# Moxfield export fixtures

Real text exports of six public Commander decks, retrieved 2026-10-25 from each
Moxfield deck page's Export dialog ("Copy Plain Text"; Najeela also "Copy for
Moxfield"). They pin the format `src/deck/interop.ts` documents and are saved byte
for byte (LF, no trailing newline). `test/interop-moxfield.test.ts` uses the Kinnan
plain and Najeela printing exports; browser-checks seeds the plain-text files.

| Files | Deck | What it exercises |
| --- | --- | --- |
| `rograkh-silas.*` | https://moxfield.com/decks/79hYZQUBdUaA9xD8zLX4vQ | trailing partner-commander block |
| `bombadil-jegantha.*` | https://moxfield.com/decks/akQNNhWe1EO_3gSwpak05g | companion under SIDEBOARD: |
| `stickers-attractions.*` | https://moxfield.com/decks/w7hBUm_lL0qd8dYa9_N3mg | ATTRACTIONS: and STICKERS: boards |
| `precon-hail-caesar.*` | https://moxfield.com/decks/hvwCjVVsAEOBXY5fP57jzQ | a split card written with one slash |
| `najeela.*` | https://moxfield.com/decks/jT8Y9X4tlUmeNZ2AjkD1Vg | sideboard; printing export with foils, collector number 1492★ |
| `kinnan-midrange.*` | https://moxfield.com/decks/OYpsy84lZU-HPrQiW9hmdQ | sideboard, 100-card list with commander |
