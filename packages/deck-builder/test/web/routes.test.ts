import { test } from "node:test";
import assert from "node:assert/strict";
import { href, isPublicRoute, parseRoute, type Route } from "../../web/src/app/routes.ts";

test("every route is a path that parses back to itself (spec §11.1)", () => {
  const routes: Route[] = [
    { page: "decks" }, { page: "inbox" }, { page: "memories" }, { page: "profile" }, { page: "activity" },
    { page: "settings" }, { page: "design" }, { page: "signin" }, { page: "privacy" }, { page: "terms" },
    { page: "deck", deckId: 7, review: false }, { page: "deck", deckId: 7, review: true }, { page: "playtest", deckId: 7 },
    { page: "shared", slug: "Ab3dE6gH9k" }, { page: "user", handle: "second_user" },
  ];
  for (const route of routes) {
    const path = href(route);
    assert.ok(path.startsWith("/") && !path.includes("#"), path);
    assert.deepEqual(parseRoute(path), route, path);
  }
  assert.equal(href({ page: "decks" }), "/");
  assert.deepEqual(parseRoute("/decks"), { page: "decks" });
  assert.equal(href({ page: "deck", deckId: 3, review: true }), "/deck/3/review");
  assert.equal(href({ page: "shared", slug: "Ab3dE6gH9k" }), "/d/Ab3dE6gH9k");
  assert.equal(href({ page: "user", handle: "owner" }), "/u/owner");
});

test("unknown paths are missing, trailing slashes are ignored, shared pages are public", () => {
  assert.deepEqual(parseRoute("/nowhere"), { page: "missing" });
  assert.deepEqual(parseRoute("/deck/x"), { page: "missing" });
  assert.deepEqual(parseRoute("/deck/4/"), { page: "deck", deckId: 4, review: false });
  assert.ok(isPublicRoute(parseRoute("/d/Ab3dE6gH9k")));
  assert.ok(isPublicRoute(parseRoute("/u/owner")));
  assert.ok(!isPublicRoute(parseRoute("/deck/1")));
  assert.ok(!isPublicRoute(parseRoute("/inbox")));
});
