import { test, describe } from "node:test";
import assert from "node:assert/strict";
import type { DatabaseSync } from "node:sqlite";
import { OWNER, PermissionError } from "../src/actor.ts";
import { listActivity } from "../src/activity.ts";
import { CardResolutionError } from "../src/cards.ts";
import { listLog, unreviewedCount } from "../src/deck/log.ts";
import { forkDeck } from "../src/deck/fork.ts";
import { acceptItems, pendingItemIds, rejectItems } from "../src/deck/proposals.ts";
import {
  createSlot,
  getDeck,
  listDecks,
  setDeckCategory,
} from "../src/deck/service.ts";
import { getProfile, listDocumentEdits, setProfile } from "../src/documents.ts";
import { setFocus } from "../src/focus.ts";
import { listMemories, remember, setBudgets } from "../src/memory.ts";
import { PLAYBOOK } from "../src/mcp/playbook.ts";
import { TOOLS, callTool } from "../src/mcp/tools.ts";
import { AGENT, add, newDeck, testDb } from "./helpers.ts";

const call = (db: DatabaseSync, name: string, args: unknown = {}) =>
  callTool({ db, actor: AGENT }, name, args);
const text = (db: DatabaseSync, name: string, args: unknown = {}) => call(db, name, args).text;

function seeded(db: DatabaseSync, status: "theorycrafted" | "paper" = "paper") {
  const id = newDeck(db, status, `Teferi ${status} ${Math.random().toString(36).slice(2, 7)}`);
  add(db, id, "id-teferi", { role: "commander" });
  add(db, id, "id-counterspell");
  return id;
}

describe("the tool list (spec §10.3)", () => {
  test("thirty tools, uniquely named, every one described", () => {
    assert.equal(TOOLS.length, 30);
    assert.equal(new Set(TOOLS.map((t) => t.name)).size, 30);
    for (const t of TOOLS) assert.ok(t.description.length > 80, t.name);
    assert.equal(TOOLS[0].name, "start_session");
  });
  test("no tool lets an agent rule, undo or change status", () => {
    for (const t of TOOLS)
      assert.ok(!/^(accept|reject|rule|undo|set_deck_status|import)/.test(t.name), t.name);
    assert.deepEqual(TOOLS.filter((t) => t.destructive).map((t) => t.name), ["delete_deck"]);
    // Physical assembly is ordinary tag organization, with no dedicated tool.
    assert.ok(TOOLS.every((t) => t.name !== "paper_status"));
  });
  test("bad arguments are explained, not thrown as a type error", () => {
    const db = testDb();
    assert.throws(() => call(db, "get_deck", {}), /Invalid arguments for get_deck: deck/);
    assert.throws(() => call(db, "remember", { text: "x", scope: "everywhere" }), /Invalid arguments/);
    assert.throws(() => call(db, "no_such_tool"), /Unknown tool/);
  });
});

describe("deck management tools", () => {
  test("renames editable decks, while lifecycle changes are owner-only", () => {
    const db = testDb();
    for (const category of ["theorycrafted", "paper"] as const) {
      const id = seeded(db, category);
      const before = getDeck(db, id);
      const name = `Managed ${category}`;
      assert.match(text(db, "rename_deck", { deck: id, name }), /Renamed deck/);
      assert.equal(getDeck(db, id).deck.category, category);
      assert.equal(getDeck(db, id).deck.revision, before.deck.revision);
      assert.deepEqual(getDeck(db, id).cards, before.cards);
      assert.equal(listActivity(db)[0].actor, "test-agent");
      assert.throws(() => call(db, "set_deck_category", { deck: id, category: "theorycrafted" }), /Unknown tool/);
    }
  });
  test("deletes a Paper deck and its records while preserving forks, global memories and attribution", () => {
    const db = testDb();
    const id = seeded(db);
    const name = getDeck(db, id).deck.name;
    const fork = forkDeck(db, AGENT, id, "Surviving fork");
    remember(db, OWNER, { text: "Deck-specific preference", scope: "deck", deck_id: id });
    const global = remember(db, OWNER, { text: "Global preference", scope: "global" });
    const result = call(db, "delete_deck", { deck: name });
    assert.equal(result.changed, true);
    assert.match(result.text, /Deleted deck .*This cannot be undone/);
    assert.throws(() => getDeck(db, id), /There is no deck/);
    assert.equal(getDeck(db, fork).deck.parent, null);
    assert.ok(getDeck(db, fork).cards.length > 0);
    assert.equal(listMemories(db, { deckId: id }).length, 0);
    assert.ok(listMemories(db, { scope: "global" }).some((m) => m.id === global.memory.id));
    const entry = listActivity(db)[0];
    assert.equal(entry.actor, "test-agent");
    assert.equal(entry.action, "delete_deck");
    assert.equal(entry.deck_id, null);
    assert.ok(entry.summary.includes(name));
    assert.throws(() => call(db, "delete_deck", { deck: id }), /There is no deck/);
  });

  test("agents create only Theorycrafted decks, with category writes rejected", () => {
    const db = testDb();
    assert.throws(() => call(db, "create_deck", { name: "Paper idea", category: "paper" }), /Invalid arguments/);
    assert.equal(listDecks(db).length, 0);
    const created = call(db, "create_deck", { name: "New idea" });
    assert.match(created.text, /lifecycle theorycrafted/);
    assert.equal(listDecks(db)[0].category, "theorycrafted");
  });
});

describe("start_session", () => {
  test("carries the playbook, profile, global memories, roster, focus and the review queue", () => {
    const db = testDb();
    const id = seeded(db);
    setProfile(db, OWNER, "Likes long games.");
    remember(db, AGENT, { text: "Avoids fast mana.", scope: "global" });
    setFocus(db, id, ["id-counterspell"]);
    add(db, id, "id-solring");
    const out = text(db, "start_session");
    assert.ok(out.startsWith(PLAYBOOK));
    assert.match(out, /# The owner's profile\nLikes long games\./);
    assert.match(out, /memory #1 \(global, inferred — a hypothesis\): Avoids fast mana\./);
    assert.match(out, new RegExp(`- deck ${id} "Teferi paper \\w+" — Teferi, Temporal Archmage · U · 3/100`));
    assert.match(out, new RegExp(`The owner has deck ${id} .* open, with these cards selected: Counterspell`));
    assert.match(out, /# Rulings you have not read\n3\./);
  });
  test("an empty app reads sensibly", () => {
    const out = text(testDb(), "start_session");
    assert.match(out, /\(empty — the owner has not written one/);
    assert.match(out, /\(no global memories yet\)/);
    assert.match(out, /\(no decks yet\)/);
    assert.match(out, /no deck open in the app/);
    assert.match(out, /# Rulings you have not read\nNone\./);
  });
  test("a global scope over budget says so and how to fix it", () => {
    const db = testDb();
    setBudgets(db, { global: 100 });
    for (let i = 0; i < 6; i++)
      remember(db, AGENT, { text: `Preference ${i}: ${"detail ".repeat(14)}`, scope: "global" });
    assert.match(text(db, "start_session"), /OVER BUDGET: \d more global memor/);
  });
});

describe("naming a deck", () => {
  const db = testDb();
  const id = newDeck(db, "paper", "Jo's Golems");
  test("by id, numeric string, exact name, or focus", () => {
    for (const ref of [id, String(id), "jo's golems"])
      assert.match(text(db, "get_deck", { deck: ref, sections: ["state"] }), /# Deck \d+ "Jo's Golems"/);
    assert.throws(() => call(db, "get_deck", { deck: "focus" }), /no deck open.*\n- deck \d+ "Jo's Golems"/s);
    setFocus(db, id);
    assert.match(text(db, "get_deck", { deck: "focus", sections: ["state"] }), /Jo's Golems/);
  });
  test("an unknown deck lists the ones that exist", () => {
    assert.throws(() => call(db, "get_deck", { deck: "Golems" }), /no deck named "Golems"\. The decks are:\n- deck/);
    assert.throws(() => call(db, "get_deck", { deck: 999 }), /There is no deck 999/);
  });
});

describe("reading a deck", () => {
  const db = testDb();
  const id = seeded(db);
  const ramp = createSlot(db, id, "Ramp", 3, 5);
  add(db, id, "id-solring", { slotId: ramp });
  test("the counts are stated, grouped by slot, with oracle text", () => {
    const out = text(db, "get_deck", { deck: id });
    assert.match(out, /lifecycle paper · revision 3 · identity U · 3\/100 cards/);
    assert.match(out, /Paper: change its cards only with propose_changes\./);
    assert.match(out, /### Ramp \(1 primary; target 3–5; 0 secondary\)\n- Sol Ring \{1\} — Artifact\n    \{T\}: Add \{C\}\{C\}\./);
    assert.match(out, /- Cards: 3\/100 \(97 short\)/);
    assert.match(out, /- Ramp: 1 primary \(target 3–5\) — UNDER by 2; 0 secondary; 1 total coverage/);
    assert.match(out, /PROBLEM: Deck is 3\/100/);
    assert.match(out, /note: Ramp is under target/);
  });
  test("sections and card_text make a re-read cheap", () => {
    const full = text(db, "get_deck", { deck: id });
    const names = text(db, "get_deck", { deck: id, sections: ["list"], card_text: false });
    assert.ok(names.length < full.length / 3);
    assert.ok(!names.includes("Add {C}{C}"));
    assert.ok(names.includes("- Sol Ring {1} — Artifact"));
    assert.ok(!names.includes("## Checks"));
  });

});

describe("searching and looking up cards (spec §4.2, §7.2, §8.3)", () => {
  const db = testDb();
  const id = seeded(db);
  const other = newDeck(db, "paper", "Other deck");
  add(db, other, "id-solring");
  // The owner has rejected Sol Ring here before, and a memory is linked to it.
  const pid = Number(call(db, "propose_changes", {
    deck: id,
    items: [{ action: "add", card: "Sol Ring", reason: "Ramp." }],
  }).text.match(/Proposal #(\d+)/)![1]);
  rejectItems(db, OWNER, pendingItemIds(db, pid), "Too fast for this pod.");
  remember(db, AGENT, { text: "[[Sol Ring]] is banned in the owner's playgroup.", scope: "global", basis: "stated" });

  test("with a deck, results stay inside its identity and carry rulings and memories", () => {
    const out = text(db, "search_cards", { query: "mv<=2", deck: id });
    assert.ok(out.includes("Counterspell {U}{U} — Instant  (already in this deck)"));
    assert.ok(!out.includes("Llanowar Elves"));
    assert.match(out, /- Sol Ring \{1\} — Artifact\n    \{T\}: Add \{C\}\{C\}\.\n  - ruling #\d+ at revision 2: the owner rejected adding it — the owner's reason: "Too fast for this pod\." — it was proposed because: "Ramp\."\n  - memory #1 \(global, stated\)/);
  });
  test("other decks are named only when the call asks", () => {
    assert.ok(!text(db, "search_cards", { query: "Sol Ring", deck: id }).includes("in your decks"));
    assert.match(
      text(db, "search_cards", { query: "Sol Ring", deck: id, include_my_decks: true }),
      /- in your decks: Other deck/,
    );
    assert.match(text(db, "get_cards", { cards: ["Counterspell", "Sol Ring"], include_my_decks: true }), /in your decks: Teferi paper \w+\n.*in your decks: Other deck$/s);
  });
  test("identity can be lifted, and an empty result says what to try", () => {
    assert.match(text(db, "search_cards", { query: "Llanowar", deck: id }), /No cards match.*ignore_identity: true/);
    assert.match(text(db, "search_cards", { query: "Llanowar", deck: id, ignore_identity: true }), /Llanowar Elves/);
    assert.throws(() => call(db, "search_cards", { query: "rarity:mythic" }), /could not be read.*Supported keys/);
  });
  test("results are capped and say when there are more", () => {
    const out = text(db, "search_cards", { query: "t:creature", limit: 3 });
    assert.match(out, /^3 result\(s\) .* there are more; narrow the query/);
  });
  test("get_cards checks names without failing the lookup", () => {
    const out = text(db, "get_cards", { cards: ["Sol Ringz", "Counterspell", "Rampant Growth"], deck: id });
    assert.match(out, /- "Sol Ringz" is NOT a card name\. Closest names: Sol Ring/);
    assert.match(out, /- Counterspell \{U\}\{U\} — Instant  \(already in this deck\)/);
    assert.match(out, /- Rampant Growth \{1\}\{G\} — Sorcery\n/);
  });
});

describe("Theorycrafted cards are edited directly (spec §5.2)", () => {
  test("create, fill, set commanders and write the brief in one call", () => {
    const db = testDb();
    const r = call(db, "create_deck", {
      name: "Elves",
      commanders: ["Atraxa, Praetors' Voice"],
      list: "1 Llanowar Elves\n2 Forest\n1 Sol Ring",
      thesis: "Go wide with [[Llanowar Elves]].",
    });
    assert.equal(r.changed, true);
    assert.match(r.text, /Created Theorycrafted deck 1 "Elves" \(lifecycle theorycrafted\)\.\nInitial thesis proposed as edit #1; awaiting the owner\.\nDeck 1 "Elves" is now at revision 2: 5\/100 cards \(95 short\), 2 lands\./);
    const view = getDeck(db, 1);
    assert.deepEqual([view.deck.category, view.deck.color_identity], ["theorycrafted", "WUBG"]);
    assert.deepEqual(listActivity(db).map((a) => a.action), ["create_deck"]);
  });
  test("one bad name and nothing is created", () => {
    const db = testDb();
    assert.throws(
      () => call(db, "create_deck", { name: "Bad", list: "1 Sol Ring\n1 Signet of the Adeptus" }),
      CardResolutionError,
    );
    assert.throws(() => call(db, "create_deck", { name: "Bad", commanders: ["Atraxa"] }), CardResolutionError);
    assert.equal(listDecks(db).length, 0);
  });
  test("edit_cards adds, removes, re-seats the commander and reports the new state", () => {
    const db = testDb();
    const id = seeded(db, "theorycrafted");
    createSlot(db, id, "Ramp");
    const out = text(db, "edit_cards", {
      deck: id,
      add: [{ card: "Sol Ring", slot: "ramp" }, { card: "Forest", quantity: 3 }],
      remove: [{ card: "Counterspell" }],
      commanders: ["Atraxa, Praetors' Voice"],
      note: "Rebuild around Atraxa",
    });
    assert.match(out, /Done \(added 2, removed 1, set commanders\)/);
    assert.match(out, /is now at revision 3: 6\/100 cards/);
    const view = getDeck(db, id);
    assert.deepEqual(
      view.cards.filter((c) => c.role === "commander").map((c) => c.name),
      ["Atraxa, Praetors' Voice"],
    );
    // The former commander stays in the 99.
    assert.equal(view.cards.find((c) => c.oracle_id === "id-teferi")!.role, "card");
    assert.equal(listLog(db, { deckId: id })[0].summary, "Rebuild around Atraxa");
    assert.equal(listLog(db, { deckId: id })[0].author, "test-agent");
  });
  test("on a Paper deck it is refused, with the way forward", () => {
    const db = testDb();
    const id = seeded(db, "paper");
    assert.throws(
      () => call(db, "edit_cards", { deck: id, add: [{ card: "Sol Ring" }] }),
      (e: Error) => e instanceof PermissionError && /Use propose_changes/.test(e.message),
    );
    assert.throws(() => call(db, "edit_cards", { deck: id, thesis: "x" }), /Invalid arguments/);
    assert.equal(getDeck(db, id).computed.card_count, 2);
  });
  test("a fork of a Paper deck is Theorycrafted so the agent can rework", () => {
    const db = testDb();
    const id = seeded(db, "paper");
    const out = text(db, "fork_deck", { deck: id, name: "Teferi rework" });
    const forkId = Number(out.match(/Theorycrafted deck (\d+)/)![1]);
    text(db, "edit_cards", { deck: forkId, remove: [{ card: "Counterspell" }], add: [{ card: "Sol Ring" }] });
    assert.match(out, /propose a new thesis first/);
    const diff = text(db, "compare_decks", { deck: forkId });
    assert.match(diff, /— its parent/);
    assert.match(diff, /## Only in "Teferi rework" \(1\)\n- Sol Ring/);
    assert.match(diff, /## Only in "Teferi paper \w+" \(1\)\n- Counterspell/);
  });
});

describe("proposing on a Paper deck (spec §8)", () => {
  test("a proposal waits for the owner and reports what the deck would become", () => {
    const db = testDb();
    const id = seeded(db);
    const out = text(db, "propose_changes", {
      deck: id,
      summary: "Trade interaction for ramp",
      items: [
        { action: "cut", card: "Counterspell", reason: "Too reactive.", group: "swap" },
        { action: "add", card: "Sol Ring", reason: "[[Sol Ring]] taps for two.", group: "swap" },
        { action: "add", card: "Forest", reason: "A land." },
      ],
    });
    assert.match(out, /Proposal #1 is waiting for the owner's ruling in the app \(3 item\(s\)\)\./);
    assert.match(out, /The deck is 2\/100\. Awaiting ruling across all proposals: \+2 \/ −1; if all were accepted it would be 3\/100\./);
    assert.equal(getDeck(db, id).computed.card_count, 2);
    assert.match(text(db, "get_deck", { deck: id, sections: ["proposals"] }), /- cut Counterspell \(group "swap"\): Too reactive\./);
  });
  test("re-proposing a rejected card shows the ruling back to the agent", () => {
    const db = testDb();
    const id = seeded(db);
    text(db, "propose_changes", { deck: id, items: [{ action: "add", card: "Sol Ring", reason: "Ramp." }] });
    rejectItems(db, OWNER, pendingItemIds(db, 1), "No fast mana.");
    const out = text(db, "propose_changes", {
      deck: id,
      items: [{ action: "add", card: "Sol Ring", reason: "The curve has changed." }],
    });
    assert.match(out, /What the app already knows about these cards.*\n- Sol Ring:\n  - ruling #\d+ at revision 2: the owner rejected adding it — the owner's reason: "No fast mana\."/s);
  });
  test("an invented card in a reason, or a bad name, fails the whole proposal", () => {
    const db = testDb();
    const id = seeded(db);
    assert.throws(
      () =>
        call(db, "propose_changes", {
          deck: id,
          items: [
            { action: "add", card: "Sol Ring", reason: "Fine." },
            { action: "add", card: "Forest", reason: "Better than [[Signet of the Adeptus]]." },
          ],
        }),
      /is not a card name/,
    );
    assert.throws(
      () => call(db, "propose_changes", { deck: id, items: [{ action: "add", card: "Sol Rin", reason: "x" }] }),
      /Did you mean: Sol Ring/,
    );
    assert.equal(getDeck(db, id).computed.pending.adds, 0);
  });
  test("the brief is gated on both Paper and Theorycrafted decks", () => {
    const db = testDb();
    const active = seeded(db, "paper");
    const draft = seeded(db, "theorycrafted");
    assert.match(
      text(db, "propose_brief_edit", { deck: active, kind: "thesis", text: "Control.", reason: "Owner asked." }),
      /Proposed edit #1 is waiting/,
    );
    assert.throws(
      () => call(db, "propose_brief_edit", { deck: active, kind: "thesis", text: "Control." }),
      /Invalid arguments.*reason/,
    );
    assert.match(
      text(db, "propose_brief_edit", {
        deck: draft,
        reason: "Owner wants a control engine.",
        kind: "engine_set",
        engine_name: "Counter wall",
        description: "Say no.",
        pieces: [{ card: "Counterspell" }, { card: "Sol Ring", note: "pays for it" }],
      }),
      /Proposed edit #2 is waiting/,
    );
    assert.match(
      text(db, "propose_profile_edit", { text: "Plays control.", reason: "Three blue decks, all reactive." }),
      /Proposed edit #3 is waiting/,
    );
    assert.equal(getProfile(db).body_md, "");
    assert.equal(listDocumentEdits(db, { status: "pending" }).length, 3);
  });
});

describe("organization needs no ruling (spec §5.4)", () => {
  test("slots, filing, tags and the maybe list on a Paper deck", () => {
    const db = testDb();
    const id = seeded(db, "paper");
    const revision = getDeck(db, id).deck.revision;
    assert.match(
      text(db, "define_slots", { deck: id, slots: [{ name: "Interaction", target_min: 8, target_max: 12 }] }),
      /- Interaction: 0 primary \(target 8–12\) — UNDER by 8/,
    );
    assert.match(
      text(db, "file_cards", { deck: id, filings: [{ card: "Counterspell", slot: "interaction", add_tags: ["counter"] }] }),
      /- Interaction: 1 primary \(target 8–12\) — UNDER by 7/,
    );
    assert.match(
      text(db, "shortlist_cards", { deck: id, add: [{ card: "Sol Ring", note: "If [[Counterspell]] stays." }] }),
      /## Maybe list \(1; outside the 100\)\n- Sol Ring \{1\} — Artifact  — If \[\[Counterspell\]\] stays\./,
    );
    assert.throws(
      () => call(db, "shortlist_cards", { deck: id, add: [{ card: "Forest", note: "Like [[Forrest]]." }] }),
      /is not a card name/,
    );
    assert.throws(() => call(db, "file_cards", { deck: id, filings: [{ card: "Forest", slot: null }] }), /not in this deck/);
    assert.equal(getDeck(db, id).deck.revision, revision);
    assert.deepEqual(
      listActivity(db).map((a) => a.action).reverse(),
      ["define_slots", "file_cards", "shortlist_cards"],
    );
  });
});

describe("memories and the review queue through the tools (spec §7, §8.4)", () => {
  test("the loop: read rulings, remember what lasts, mark them read", () => {
    const db = testDb();
    const id = seeded(db);
    text(db, "propose_changes", { deck: id, items: [{ action: "add", card: "Sol Ring", reason: "Ramp." }] });
    rejectItems(db, OWNER, pendingItemIds(db, 1), "I never play fast mana.");

    const queue = text(db, "list_rulings", { unreviewed_only: true });
    assert.match(queue, /REJECTED: Rejected adding Sol Ring — the owner's reason: "I never play fast mana\." \[unreviewed\]\n  - add Sol Ring: Ramp\./);
    const rulingId = Number(queue.match(/- #(\d+) · deck[^\n]*REJECTED/)![1]);
    assert.ok(queue.endsWith(`call mark_rulings_reviewed with {"through": ${rulingId}}.`));
    // One name for a body of prose in every tool, so an agent need not guess.
    for (const tool of TOOLS) assert.ok(!("content" in tool.input), tool.name);

    const saved = text(db, "remember", {
      text: "Never plays fast mana such as [[Sol Ring]].",
      scope: "global",
      basis: "stated",
      source_ruling: rulingId,
    });
    assert.match(saved, /Saved\.\n- memory #1 \(global, stated\): Never plays fast mana/);
    assert.equal(listMemories(db, {})[0].source_log_id, rulingId);

    assert.match(text(db, "mark_rulings_reviewed", { through: rulingId }), /0 still unreviewed/);
    assert.equal(unreviewedCount(db), 0);
  });
  test("remember points at what resembles it; revise supersedes and promotes", () => {
    const db = testDb();
    const id = seeded(db);
    text(db, "remember", { text: "Cut the tutors from this list.", scope: "deck", deck: id });
    const second = text(db, "remember", { text: "Tutors keep getting cut here.", scope: "deck", deck: id });
    assert.match(second, /These existing memories resemble it.*\n- memory #1/s);
    const promoted = text(db, "revise_memory", {
      id: 1,
      action: "supersede",
      text: "Cuts tutors from every deck: wants games to differ.",
      scope: "global",
    });
    assert.match(promoted, /Memory #1 is now superseded by:\n- memory #3 \(global, inferred/);
    assert.match(text(db, "revise_memory", { id: 3, action: "confirm" }), /\(global, stated\)/);
    assert.match(text(db, "search_memories", { query: "tutors", include_retired: true }), /superseded by #3/);
    assert.throws(() => call(db, "revise_memory", { id: 2, action: "supersede" }), /needs the replacement text/);
    assert.throws(() => call(db, "remember", { text: "x", scope: "deck" }), /needs the deck/);
  });
  test("an accepted proposal reaches the agent through the same queue", () => {
    const db = testDb();
    const id = seeded(db);
    text(db, "propose_changes", { deck: id, items: [{ action: "add", card: "Forest", reason: "A land." }] });
    acceptItems(db, OWNER, pendingItemIds(db, 1));
    assert.match(text(db, "list_rulings", { deck: id, limit: 1 }), /ACCEPTED: Added Forest/);
    setDeckCategory(db, OWNER, id, "theorycrafted");
    assert.match(text(db, "get_deck", { deck: id, sections: [] }), /Theorycrafted: edit its cards directly with edit_cards\./);
  });
});

describe("statistics and export", () => {
  test("player_stats and export_deck render the app's numbers", () => {
    const db = testDb();
    const id = seeded(db, "theorycrafted");
    assert.match(text(db, "player_stats"), /computed from 0 Paper deck\(s\)/);
    setDeckCategory(db, OWNER, id, "paper");
    assert.match(text(db, "player_stats"), /computed from 1 Paper deck\(s\)/);
    assert.match(text(db, "player_stats"), /By exact identity: U ×1/);
    assert.match(text(db, "export_deck", { deck: id }), /2 card\(s\) at revision 2:\n\n1 Teferi, Temporal Archmage \[Commander\]\n1 Counterspell \[Unslotted\]/);
  });
});
