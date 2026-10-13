// Load the real JSX through the package's Vite pipeline, then inspect the
// rendered HTML. This covers component/HTTP-wire integration without a new
// runtime or DOM test dependency. Native interaction checks use the live UI.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "vite";
import { renderToStaticMarkup } from "react-dom/server";
import { createElement } from "react";
import { OWNER } from "../../src/actor.ts";
import { startPlaytest } from "../../src/playtest/service.ts";
import { add, newDeck, testDb } from "../helpers.ts";
test("table components render actual wire facts, readable state and anonymous hidden observations", async (t) => {
  const vite = await createServer({
    server: { middlewareMode: true },
    appType: "custom",
  });
  t.after(() => vite.close());
  const db = testDb();
  t.after(() => db.close());
  const deck = newDeck(db);
  add(db, deck, "id-witch");
  add(db, deck, "id-forest", { quantity: 10 });
  const { session } = startPlaytest(db, OWNER, deck, { mode: "commander" });
  const card = session.cards["id-witch"],
    object = Object.values(session.frame.objects).find(
      (o) => o.oracle_id === card.oracle_id,
    )!;
  const { TextCard } = await vite.ssrLoadModule("/src/mtg/table/TextCard.tsx");
  const { TableZone } = await vite.ssrLoadModule(
    "/src/mtg/table/TableZone.tsx",
  );
  const { CardInspection } = await vite.ssrLoadModule(
    "/src/mtg/table/CardInspection.tsx",
  );
  const observation = {
    visibility: "known",
    card,
    object: {
      ...object,
      face: 1,
      tapped: true,
      counters: { "test counter": 3 },
    },
  };
  const visible = renderToStaticMarkup(
    createElement(TextCard, { observation, full: true, selected: true }),
  );
  assert.ok(visible.includes(card.faces[1].name));
  assert.ok(visible.includes('data-selected="true"'));
  assert.ok(visible.includes('data-tapped="true"'));
  assert.ok(visible.includes("test counter: 3"));
  const engineDisplay = renderToStaticMarkup(
    createElement(TextCard, {
      observation: {
        visibility: "known",
        card,
        object: { id: "permitted-visible-object", tapped: true },
      },
    }),
  );
  assert.ok(
    engineDisplay.includes('data-object-id="permitted-visible-object"'),
  );
  assert.ok(!engineDisplay.includes("Manual "));
  const hidden = renderToStaticMarkup(
    createElement(TextCard, { observation: { visibility: "hidden" } }),
  );
  assert.ok(hidden.includes("Anonymous card"));
  assert.ok(!hidden.includes(card.name));
  assert.ok(!hidden.includes(card.oracle_id));
  const row = renderToStaticMarkup(
    createElement(
      TableZone,
      { label: "Creatures", count: 1, showHeader: false },
      createElement(TextCard, { observation }),
    ),
  );
  assert.ok(row.includes('aria-label="Creatures"'));
  assert.ok(!row.includes("<h2"));
  assert.ok(!row.includes("<h2>Creatures"));
  const inspected = renderToStaticMarkup(
    createElement(
      CardInspection,
      { observation, context: "Pinned selection" },
      createElement("p", null, "Host tools"),
    ),
  );
  assert.ok(inspected.includes("Pinned selection"));
  assert.ok(inspected.includes("Host tools"));
});
