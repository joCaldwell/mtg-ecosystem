import { test } from "node:test";
import assert from "node:assert/strict";
import { href, isPublicRoute, parseRoute, safeNext } from "../web/src/app/routes.ts";

test("home and library have separate canonical paths", () => {
  assert.deepEqual(parseRoute("/"), { page: "home" });
  assert.deepEqual(parseRoute("/decks"), { page: "decks" });
  assert.equal(href({ page: "home" }), "/");
  assert.equal(href({ page: "decks" }), "/decks");
});

test("game setup and preview retain an exact optional deck context", () => {
  for (const page of ["play", "game-preview"] as const) {
    for (const deckId of [undefined, 1, 135]) {
      const route = { page, ...(deckId === undefined ? {} : { deckId }) };
      const url = new URL(href(route), "http://example.invalid");
      assert.deepEqual(parseRoute(url.pathname, url.search), route);
      assert.equal(isPublicRoute(route), false);
    }
  }
});

test("invalid selected-deck links fail explicitly without changing the deck", () => {
  for (const path of ["/play", "/play/preview"]) {
    for (const id of ["", "0", "-1", "1.5", "1e2", "NaN", "9007199254740992", "4oops"]) {
      assert.deepEqual(parseRoute(path, `?deck=${id}`), { page: "missing" });
    }
  }
  assert.deepEqual(parseRoute("/play/not-a-game"), { page: "missing" });
});

test("existing deck, manual tool and public URLs keep their identities", () => {
  assert.deepEqual(parseRoute("/deck/23"), { page: "deck", deckId: 23, review: false });
  assert.deepEqual(parseRoute("/deck/23/playtest"), { page: "playtest", deckId: 23 });
  assert.deepEqual(parseRoute("/d/saved-slug"), { page: "shared", slug: "saved-slug" });
  assert.deepEqual(parseRoute("/u/owner"), { page: "user", handle: "owner" });
  assert.equal(href({ page: "shared", slug: "saved-slug" }), "/d/saved-slug");
  assert.equal(href({ page: "user", handle: "owner" }), "/u/owner");
  assert.equal(isPublicRoute({ page: "shared", slug: "saved-slug" }), true);
});

test("new product destinations preserve same-origin sign-in return checks", () => {
  assert.equal(safeNext("/play?deck=23"), "/play?deck=23");
  assert.equal(safeNext("/play/preview?deck=23"), "/play/preview?deck=23");
  assert.equal(safeNext("//outside.invalid/play"), "/");
  assert.equal(safeNext("https://outside.invalid/play"), "/");
});
