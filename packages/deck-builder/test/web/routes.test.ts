import { test } from "node:test";
import assert from "node:assert/strict";
import { href, isPublicRoute, parseRoute, safeNext, signInHref, type Route } from "../../web/src/app/routes.ts";

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

test("a malformed escape is a missing page, not a crash (review S9)", () => {
  assert.deepEqual(parseRoute("/d/%E0%A4%A"), { page: "missing" });
  assert.deepEqual(parseRoute("/u/%"), { page: "missing" });
  assert.deepEqual(parseRoute("/d/Ab3dE6gH9k%20"), { page: "shared", slug: "Ab3dE6gH9k " });
});

test("sign-in returns only to a path on this site (review S10)", () => {
  assert.equal(safeNext("/d/Ab3dE6gH9k"), "/d/Ab3dE6gH9k");
  assert.equal(safeNext("/u/owner?x=1#top"), "/u/owner?x=1#top");
  for (const hostile of ["//evil.example/x", "/\\evil.example", "https://evil.example", "javascript:alert(1)", "evil", "", null, "/\u0000x", "/%2F%2Fevil.example",
    // Normalisation turns these into "//evil.com" (review round 2).
    "/./\\evil.com", "/a/..\\/evil.com", "/.//evil.com", "/a/..//evil.com"])
    assert.ok(["/", "/%2F%2Fevil.example"].includes(safeNext(hostile)), String(hostile));
  assert.equal(new URL(safeNext("/%2F%2Fevil.example"), "https://site.example").host, "site.example", "an encoded slash stays a path");
  assert.equal(signInHref("/d/Ab3dE6gH9k"), "/signin?next=%2Fd%2FAb3dE6gH9k");
  assert.equal(signInHref("//evil.example"), "/signin");
  assert.equal(signInHref(), "/signin");
  for (const sneaky of ["/./\\evil.com", "/a/..\\/evil.com", "/.//evil.com"]) assert.ok(!safeNext(sneaky).startsWith("//"), sneaky);
});
