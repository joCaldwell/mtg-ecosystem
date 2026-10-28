import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { acceptedNote, changesSince, staleNote } from "../../web/src/domain/staleChanges.ts";
import { logEntry } from "./fixtures.ts";

const c = (card_name: string, action: "add" | "cut" | "role") => ({ oracle_id: card_name, card_name, action, quantity: 1, item_id: null, reason: "" });

describe("changesSince", () => {
  test("names the cards changed after the base revision, newest log first", () => {
    const log = [
      logEntry({ revision: 4, cards: [c("Test Card C", "cut")] }),
      logEntry({ revision: 3, cards: [c("Test Card A", "add"), c("Test Card B", "add")] }),
      logEntry({ revision: 2, kind: "reject", cards: [c("Test Card D", "add")] }),
      logEntry({ revision: 2, cards: [c("Test Card E", "add")] }),
    ];
    assert.equal(changesSince(log, 2), "added Test Card A and Test Card B; cut Test Card C");
    assert.equal(staleNote(changesSince(log, 2)), "The deck changed since this was proposed: added Test Card A and Test Card B; cut Test Card C.");
  });

  test("says nothing specific when the log in hand does not reach the base revision", () => {
    const log = Array.from({ length: 40 }, (_, i) => logEntry({ revision: 50 - i, cards: [c(`Card ${i}`, "add")] }));
    assert.equal(changesSince(log, 3), null);
    assert.equal(staleNote(null), "The deck changed since this was proposed.");
  });

  test("rejections and entries without cards are not changes", () => {
    assert.equal(changesSince([logEntry({ revision: 5, kind: "reject", cards: [c("Test Card A", "add")] }), logEntry({ revision: 5 })], 4), null);
    assert.equal(changesSince([], null), null);
  });
});

describe("acceptedNote", () => {
  test("names what the acceptance did, not the revision", () => {
    assert.equal(acceptedNote([c("Test Card A", "add")]), "Accepted: added Test Card A.");
    assert.equal(acceptedNote([c("Test Card A", "add"), c("Test Card B", "cut")]), "Accepted: added Test Card A; cut Test Card B.");
    assert.equal(acceptedNote([]), "Accepted.");
  });
});
