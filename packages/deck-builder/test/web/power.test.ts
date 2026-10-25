import { test } from "node:test";
import assert from "node:assert/strict";
import { bracketLabel, powerRows } from "../../web/src/domain/power.ts";
import { PowerProfileSchema, POWER_FIELDS, mapBracket } from "../../src/deck/power.ts";
import { deckState } from "./fixtures.ts";

test("the comparison preserves unknowns and uses historical Game Changers for a stale assessment", () => {
  const power = deckState().power;
  power.fields = POWER_FIELDS;
  const profile = PowerProfileSchema.parse({ intent: "social", win_turn: { min: 9, max: 11 } });
  const mapping = mapBracket(profile, 0);
  assert.equal(bracketLabel(mapping), "B2?");
  assert.equal(bracketLabel(mapBracket(PowerProfileSchema.parse({}), null)), "undetermined");
  power.assessment = { id: 1, profile, revision: 1, author: "owner", author_display: "owner", author_user_id: 1, author_via: "ui", author_label: "ui", owner_touched: true, created_at: "2026-10-11 12:00:00", stale: true,
    evidence: "list", confidence: "tentative", notes: "Estimate", mapping, game_changers: { count: 0, unknown_cards: 0, cards: [] } };
  power.game_changers = { count: 3, unknown_cards: 0, cards: [] };
  const rows = powerRows(power);
  assert.equal(rows.find(r => r.field === "Game Changers")!.current, "0");
  assert.equal(rows.find(r => r.field === "Consistency")!.current, "Unknown");
  assert.equal(rows.find(r => r.field === "Usual win range")!.current, "9–11");
  assert.equal(rows.find(r => r.field === "Usual win range")!.target, "—");
});
