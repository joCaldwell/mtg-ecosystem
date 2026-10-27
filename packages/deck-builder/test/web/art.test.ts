import { test } from "node:test";
import assert from "node:assert/strict";
import { artCredit, creditedArt, isArtCrop } from "../../web/src/domain/art.ts";
import { nextFold } from "../../web/src/domain/artFold.ts";

const crop = "https://cards.scryfall.io/art_crop/front/3/b/3b951e0c-a4dd-4a20-87c6-eaa947e33aa4.jpg?1783937085";

test("only Scryfall's own art crops qualify, by parsed host and path", () => {
  assert.equal(isArtCrop(crop), true);
  for (const url of [
    "http://cards.scryfall.io/art_crop/front/a.jpg",
    "https://cards.scryfall.io/large/front/a.jpg",
    "https://cards.scryfall.io/normal/art_crop/a.jpg",
    "https://cards.scryfall.io.example.com/art_crop/a.jpg",
    "https://evil.example/art_crop/?u=https://cards.scryfall.io/art_crop/",
    "https://cards.scryfall.io:8443/art_crop/a.jpg",
    "not a url",
  ]) assert.equal(isArtCrop(url), false, url);
});

test("credited art needs a named artist, keeps at most two, and names the front face", () => {
  const art = creditedArt([
    { oracle_id: "a", name: "Front // Back", image_art_crop: crop, artist: "  Someone  " },
    { oracle_id: "b", name: "No artist", image_art_crop: crop, artist: " " },
    { oracle_id: "c", name: "No crop", image_art_crop: null, artist: "Someone" },
    { oracle_id: "d", name: "Second", image_art_crop: crop, artist: "Another" },
    { oracle_id: "e", name: "Third", image_art_crop: crop, artist: "Third" },
  ]);
  assert.deepEqual(art.map((c) => [c.oracle_id, c.name, c.artist]), [["a", "Front", "Someone"], ["d", "Second", "Another"]]);
  assert.deepEqual(creditedArt(null), []);
  assert.equal(artCredit(art[0]!, true), "Art by Someone");
  assert.equal(artCredit(art[0]!, false), "Art by Someone · Front");
});

test("banner hysteresis prevents repeated folds near the threshold and guards short lists", () => {
  const space = { threshold: 48, expandThreshold: 16, band: 72 };
  const list = (scrollTop: number, overflow: number) => ({ scrollTop, clientHeight: 500, scrollHeight: 500 + overflow });
  assert.equal(nextFold(false, list(30, 400), space), false, "above the threshold");
  assert.equal(nextFold(false, list(60, 400), space), true, "past it, with room");
  assert.equal(nextFold(true, list(48, 400), space), true, "reversing through the collapse point stays compact");
  assert.equal(nextFold(true, list(17, 400), space), true, "just outside the return threshold stays compact");
  assert.equal(nextFold(true, list(16, 400), space), false, "near the top restores the art");
  // 100 px of overflow loses 72 when the band folds: the list would clamp to
  // 28, under the threshold, and unfold again. It stays as it is instead.
  assert.equal(nextFold(false, list(60, 100), space), false, "a short list keeps its banner");
  assert.equal(nextFold(true, list(28, 28), space), true, "a browser clamp into the dead band does not unfold it");
  assert.equal(nextFold(false, list(60, 121), space), true, "just enough room folds");
  let folded = false;
  let transitions = 0;
  for (const top of [60, 45, 55, 30, 17, 16, 30, 45]) {
    const next = nextFold(folded, list(top, 400), space);
    if (next !== folded) transitions++;
    folded = next;
  }
  assert.equal(transitions, 2, "one fold and one expansion despite repeated crossings");
});
