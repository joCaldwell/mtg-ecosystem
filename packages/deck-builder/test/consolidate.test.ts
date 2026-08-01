import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { openDb, getRetentionN, setRetentionN } from "../src/db.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { addCard, createDeck } from "../src/deck/service.ts";
import { createProposal, listProposals } from "../src/deck/proposals.ts";
import { getBrief, listBriefEdits, updateBrief } from "../src/deck/brief.ts";
import { assembleContext, pairToolCalls } from "../src/agent/context.ts";
import { markCompacted, residentMessages } from "../src/agent/chatStore.ts";
import { contextMeter } from "../src/agent/meter.ts";
import {
  KEEP_RECENT_MESSAGES,
  acceptConsolidation,
  listConsolidations,
  rejectConsolidation,
  runConsolidation,
} from "../src/agent/consolidate.ts";
import type { ChatMessage, ChatTransport } from "../src/agent/llm.ts";

const db = openDb(":memory:");
ingestCards(db, FIXTURES);

function jsonTransport(payloads: string[]): { transport: ChatTransport; prompts: ChatMessage[][] } {
  const queue = [...payloads];
  const prompts: ChatMessage[][] = [];
  return {
    prompts,
    transport: async (req) => {
      prompts.push([...req.messages]);
      const content = queue.shift();
      if (content === undefined) throw new Error("transport exhausted");
      return { message: { role: "assistant", content }, finish_reason: "stop" };
    },
  };
}

// Seed a deck plus a chat long enough to have a compaction zone.
function seedChat(name: string, extraMessages = 0): number {
  const id = createDeck(db, name);
  addCard(db, id, "id-teferi", { role: "commander" });
  addCard(db, id, "id-solring");
  addCard(db, id, "id-counterspell");
  updateBrief(db, id, { thesis: "Untap value.", constraints_md: "No infinite combos." });

  const total = KEEP_RECENT_MESSAGES + 4 + extraMessages;
  const stmt = db.prepare(
    "INSERT INTO chat_messages (deck_id, role, content_json) VALUES (?, ?, ?)",
  );
  for (let i = 0; i < total; i++) {
    const role = i % 2 === 0 ? "user" : "assistant";
    stmt.run(id, role, JSON.stringify({ role, content: `message ${i} about [[Sol Ring]]` }));
  }
  return id;
}

const OK_PAYLOAD = JSON.stringify({
  summary: "The owner wants [[Sol Ring]] kept and is uninterested in counterspells.",
  discarded: ["an abandoned plan to go wide with tokens"],
  brief_edits: [
    {
      section: "constraints",
      content: "No infinite combos. No counterspells.",
      rationale: "Every counterspell suggested so far has been cut.",
    },
  ],
  rescued: [
    {
      fact: "The owner's playgroup bans fast mana past turn 3",
      should_have_been: "brief",
      why: "It was only ever said in chat.",
    },
  ],
});

describe("consolidate (spec §11)", () => {
  test("produces a pending consolidation and gated brief edits; nothing is applied yet", async () => {
    const id = seedChat("Consolidate basic");
    const { transport, prompts } = jsonTransport([OK_PAYLOAD]);
    const c = await runConsolidation(db, id, transport);

    assert.equal(c.status, "pending");
    assert.equal(c.message_count, 4);
    assert.match(c.summary, /Sol Ring/);
    assert.deepEqual(c.rescued.map((r) => r.should_have_been), ["brief"]);

    // The brief edit exists only as a pending, source-tagged proposal.
    const edits = listBriefEdits(db, id, "pending");
    assert.equal(edits.length, 1);
    assert.equal(edits[0].source, "consolidation");
    assert.equal(getBrief(db, id).constraints_md, "No infinite combos."); // untouched

    // Nothing left context yet.
    const { compacted } = db
      .prepare("SELECT COUNT(*) compacted FROM chat_messages WHERE deck_id = ? AND compacted_at IS NOT NULL")
      .get(id) as { compacted: number };
    assert.equal(compacted, 0);

    // The model was shown the brief and the messages, and told the deck is safe.
    assert.match(prompts[0][0].content!, /condensing conversation, not data/);
    assert.match(prompts[0][1].content!, /Untap value/);
  });

  test(`the last ${KEEP_RECENT_MESSAGES} messages are never compacted`, async () => {
    const id = seedChat("Consolidate zone");
    const { transport } = jsonTransport([OK_PAYLOAD]);
    const c = await runConsolidation(db, id, transport);
    acceptConsolidation(db, id, c.id);

    const resident = db
      .prepare("SELECT COUNT(*) n FROM chat_messages WHERE deck_id = ? AND compacted_at IS NULL")
      .get(id) as { n: number };
    assert.equal(resident.n, KEEP_RECENT_MESSAGES);
  });

  test("a short chat has nothing to consolidate", async () => {
    const id = createDeck(db, "Consolidate short");
    db.prepare("INSERT INTO chat_messages (deck_id, role, content_json) VALUES (?, 'user', ?)").run(
      id,
      JSON.stringify({ role: "user", content: "hi" }),
    );
    const { transport } = jsonTransport([OK_PAYLOAD]);
    await assert.rejects(() => runConsolidation(db, id, transport), /Nothing to consolidate/);
  });

  test("only one consolidation may await a ruling at a time", async () => {
    const id = seedChat("Consolidate single");
    const { transport } = jsonTransport([OK_PAYLOAD, OK_PAYLOAD]);
    await runConsolidation(db, id, transport);
    await assert.rejects(() => runConsolidation(db, id, transport), /already awaiting your ruling/);
  });
});

describe("consolidation hard boundary (spec §11 — enforced in code)", () => {
  test("accepting touches only the transcript: deck, brief, proposals and filters are untouched", async () => {
    const id = seedChat("Consolidate boundary");
    createProposal(db, id, [{ action: "add", oracle_id: "id-goyf", rationale: "keep me" }]);

    const snapshot = () => ({
      cards: db
        .prepare("SELECT oracle_id, quantity, slot_id, role FROM deck_cards WHERE deck_id = ? ORDER BY oracle_id")
        .all(id),
      brief: getBrief(db, id),
      proposals: listProposals(db, id, "open"),
      log: db.prepare("SELECT * FROM decision_log WHERE deck_id = ? ORDER BY id").all(id),
      filters: db.prepare("SELECT * FROM hard_filters WHERE deck_id = ?").all(id),
      notes: db.prepare("SELECT * FROM card_notes WHERE deck_id = ?").all(id),
      playtest: db.prepare("SELECT * FROM playtest_notes WHERE deck_id = ?").all(id),
      revision: db.prepare("SELECT revision FROM decks WHERE id = ?").get(id),
    });

    const { transport } = jsonTransport([OK_PAYLOAD]);
    const c = await runConsolidation(db, id, transport);
    const before = snapshot();
    acceptConsolidation(db, id, c.id);
    const after = snapshot();

    assert.deepEqual(after, before);
  });

  test("compaction is non-destructive — messages stay on disk", async () => {
    const id = seedChat("Consolidate nondestructive");
    const total = () =>
      (db.prepare("SELECT COUNT(*) n FROM chat_messages WHERE deck_id = ?").get(id) as { n: number })
        .n;
    const before = total();

    const { transport } = jsonTransport([OK_PAYLOAD]);
    const c = await runConsolidation(db, id, transport);
    acceptConsolidation(db, id, c.id);

    assert.equal(total(), before);
    const compacted = db
      .prepare("SELECT COUNT(*) n FROM chat_messages WHERE deck_id = ? AND compacted_at IS NOT NULL")
      .get(id) as { n: number };
    assert.equal(compacted.n, 4);
  });

  test("rejecting leaves the transcript entirely alone", async () => {
    const id = seedChat("Consolidate reject");
    const { transport } = jsonTransport([OK_PAYLOAD]);
    const c = await runConsolidation(db, id, transport);
    rejectConsolidation(db, id, c.id);

    const compacted = db
      .prepare("SELECT COUNT(*) n FROM chat_messages WHERE deck_id = ? AND compacted_at IS NOT NULL")
      .get(id) as { n: number };
    assert.equal(compacted.n, 0);
    assert.equal(assembleContext(db, id, 30).transcript.length, KEEP_RECENT_MESSAGES + 4);
    // The brief edit it proposed survives as an ordinary pending edit.
    assert.equal(listBriefEdits(db, id, "pending").length, 1);
  });

  test("a summary naming a nonexistent card is bounced, then refused — never stored", async () => {
    const id = seedChat("Consolidate lint");
    const bad = JSON.stringify({ summary: "The owner loves [[Mana Drainn]].", rescued: [] });
    const { transport, prompts } = jsonTransport([bad, bad]);
    await assert.rejects(() => runConsolidation(db, id, transport), /Mana Drainn/);
    assert.equal(prompts.length, 2);
    assert.match(prompts[1].at(-1)!.content!, /do not resolve/);
    assert.equal(listConsolidations(db, id).length, 0);
  });

  test("a corrected second attempt is accepted", async () => {
    const id = seedChat("Consolidate retry");
    const { transport, prompts } = jsonTransport(["not json at all", OK_PAYLOAD]);
    const c = await runConsolidation(db, id, transport);
    assert.equal(c.status, "pending");
    assert.match(prompts[1].at(-1)!.content!, /not valid JSON/);
  });

  test("brief edits naming nonexistent cards are dropped rather than gated", async () => {
    const id = seedChat("Consolidate edit lint");
    const { transport } = jsonTransport([
      JSON.stringify({
        summary: "Fine.",
        brief_edits: [
          { section: "thesis", content: "Win with [[Mana Drainn]].", rationale: "r" },
          { section: "constraints", content: "Keep [[Sol Ring]].", rationale: "r" },
        ],
        rescued: [],
      }),
    ]);
    await runConsolidation(db, id, transport);
    const edits = listBriefEdits(db, id, "pending");
    assert.equal(edits.length, 1);
    assert.match(edits[0].payload.content ?? "", /Sol Ring/);
  });
});

describe("compacted context assembly (spec §11)", () => {
  test("compacted messages leave context and the approved summary stands in for them", async () => {
    const id = seedChat("Consolidate context");
    const before = assembleContext(db, id, 30);
    assert.equal(before.transcript.length, KEEP_RECENT_MESSAGES + 4);

    const { transport } = jsonTransport([OK_PAYLOAD]);
    const c = await runConsolidation(db, id, transport);
    acceptConsolidation(db, id, c.id);

    const after = assembleContext(db, id, 30);
    // 4 messages replaced by one summary message at the head.
    assert.equal(after.transcript.length, KEEP_RECENT_MESSAGES + 1);
    assert.equal(after.transcript[0].role, "system");
    assert.match(after.transcript[0].content!, /<compacted_history messages="4">/);
    assert.match(after.transcript[0].content!, /uninterested in counterspells/);
    assert.match(after.transcript[0].content!, /source of truth/);
  });

  test("a second consolidation supersedes the first, so only one summary is ever resident", async () => {
    const id = seedChat("Consolidate supersede");
    const first = jsonTransport([OK_PAYLOAD]);
    acceptConsolidation(db, id, (await runConsolidation(db, id, first.transport)).id);

    // The chat keeps going, so a second compaction zone forms.
    const stmt = db.prepare(
      "INSERT INTO chat_messages (deck_id, role, content_json) VALUES (?, 'user', ?)",
    );
    for (let i = 0; i < 6; i++)
      stmt.run(id, JSON.stringify({ role: "user", content: `later message ${i}` }));

    const second = jsonTransport([
      JSON.stringify({ summary: "Second pass, folding in the first.", rescued: [] }),
    ]);
    const c2 = await runConsolidation(db, id, second.transport);
    // The prior summary is handed to the model so it can fold it in.
    assert.match(second.prompts[0][1].content!, /Existing compaction summary/);
    assert.match(second.prompts[0][1].content!, /uninterested in counterspells/);

    acceptConsolidation(db, id, c2.id);
    const ctx = assembleContext(db, id, 30);
    const summaries = ctx.transcript.filter((m) => m.content?.includes("<compacted_history"));
    assert.equal(summaries.length, 1);
    assert.match(summaries[0].content!, /Second pass/);
  });
});

// A transcript whose compaction boundary falls exactly between an assistant's
// tool_calls and the tool message answering it — the split that leaves an
// orphaned `tool` row at the head of resident context.
function seedChatSplitByToolCall(name: string): number {
  const id = createDeck(db, name);
  addCard(db, id, "id-teferi", { role: "commander" });
  const stmt = db.prepare(
    "INSERT INTO chat_messages (deck_id, role, content_json) VALUES (?, ?, ?)",
  );
  const push = (m: ChatMessage) => stmt.run(id, m.role, JSON.stringify(m));

  push({ role: "user", content: "opening question" });
  push({ role: "assistant", content: "opening answer about [[Sol Ring]]" });
  push({ role: "user", content: "follow-up" });
  // Index 3 — the last message of the zone.
  push({
    role: "assistant",
    content: null,
    tool_calls: [{ id: "call_split", type: "function", function: { name: "search_cards", arguments: "{}" } }],
  });
  // Index 4 — first message left resident, and an orphan if the cut stands.
  push({ role: "tool", tool_call_id: "call_split", content: "1 result(s)" });
  for (let i = 0; i < KEEP_RECENT_MESSAGES - 1; i++)
    push({ role: i % 2 === 0 ? "assistant" : "user", content: `later message ${i}` });
  return id;
}

describe("tool-call pairing across the compaction boundary", () => {
  test("the boundary never splits an assistant's tool_calls from its results", async () => {
    const id = seedChatSplitByToolCall("Consolidate tool split");
    const { transport } = jsonTransport([
      JSON.stringify({ summary: "The owner asked about [[Sol Ring]].", rescued: [] }),
    ]);
    const c = await runConsolidation(db, id, transport);
    acceptConsolidation(db, id, c.id);

    // The zone swallowed the tool result rather than stopping in front of it.
    assert.equal(c.message_count, 5);
    const ctx = assembleContext(db, id, 30);
    const afterSummary = ctx.transcript.slice(1);
    assert.notEqual(afterSummary[0].role, "tool");
    assert.equal(afterSummary.filter((m) => m.role === "tool").length, 0);
  });

  test("a window already split by an older compaction is repaired on read", () => {
    const id = seedChatSplitByToolCall("Consolidate tool split legacy");
    // Reproduce the pre-fix cut directly: compact through the assistant only.
    const rows = db
      .prepare("SELECT id FROM chat_messages WHERE deck_id = ? ORDER BY id")
      .all(id) as unknown as Array<{ id: number }>;
    markCompacted(db, id, rows[3].id);

    const resident = residentMessages(db, id).map((r) => r.message);
    assert.equal(resident[0].role, "tool", "precondition: the stored window is split");

    const ctx = assembleContext(db, id, 30);
    assert.equal(
      ctx.transcript.filter((m) => m.role === "tool" && m.tool_call_id === "call_split").length,
      0,
      "the orphaned tool result never reaches the provider",
    );
    // Repaired on read only — the transcript on disk is untouched (spec §11).
    assert.equal(residentMessages(db, id)[0].message.role, "tool");
  });

  test("an assistant tool_call whose results never landed is dropped, keeping its prose", () => {
    const paired = pairToolCalls([
      { role: "user", content: "go" },
      {
        role: "assistant",
        content: "Looking that up.",
        tool_calls: [
          { id: "call_ok", type: "function", function: { name: "search_cards", arguments: "{}" } },
          { id: "call_dead", type: "function", function: { name: "search_cards", arguments: "{}" } },
        ],
      },
      { role: "tool", tool_call_id: "call_ok", content: "1 result(s)" },
    ]);
    const assistant = paired.find((m) => m.role === "assistant")!;
    assert.deepEqual(assistant.tool_calls?.map((c) => c.id), ["call_ok"]);
    assert.equal(assistant.content, "Looking that up.");
    assert.equal(paired.filter((m) => m.role === "tool").length, 1);
  });

  test("a turn that died before any result landed leaves no dangling call", () => {
    const paired = pairToolCalls([
      { role: "user", content: "go" },
      {
        role: "assistant",
        content: null,
        tool_calls: [
          { id: "call_dead", type: "function", function: { name: "search_cards", arguments: "{}" } },
        ],
      },
      { role: "user", content: "still there?" },
    ]);
    assert.deepEqual(
      paired.map((m) => m.role),
      ["user", "user"],
    );
  });
});

describe("segmented context meter (spec §11)", () => {
  test("reports every segment separately, not one number", () => {
    const id = seedChat("Meter segments");
    const m = contextMeter(db, id, 30);
    assert.deepEqual(
      m.segments.map((s) => s.key),
      ["rules", "brief", "slots", "decklist", "computed", "pending", "log", "transcript", "tail_restate"],
    );
    assert.ok(m.est_tokens > 0);
    assert.equal(
      m.est_tokens,
      m.segments.reduce((n, s) => n + s.est_tokens, 0),
    );
    assert.ok(m.segments.every((s) => s.share >= 0 && s.share <= 1));
    // The two unbounded segments are the ones the levers act on.
    assert.deepEqual(
      m.segments.filter((s) => s.behavior === "grows").map((s) => s.key),
      ["log", "transcript"],
    );
  });

  test("the transcript segment shrinks after compaction", async () => {
    const id = seedChat("Meter compaction", 20);
    const before = contextMeter(db, id, 30);
    const { transport } = jsonTransport([OK_PAYLOAD]);
    acceptConsolidation(db, id, (await runConsolidation(db, id, transport)).id);
    const after = contextMeter(db, id, 30);

    assert.ok(after.segments.find((s) => s.key === "transcript")!.est_tokens <
      before.segments.find((s) => s.key === "transcript")!.est_tokens);
    assert.ok(after.compacted_messages > 0);
    assert.equal(after.has_active_summary, true);
  });

  test("a quiet deck gets an explicit all-clear rather than silence", () => {
    const id = createDeck(db, "Meter quiet");
    const m = contextMeter(db, id, 30);
    assert.equal(m.advice.length, 1);
    assert.match(m.advice[0].message, /No compaction needed/);
  });
});

describe("decision-log retention N (spec §12)", () => {
  test("is a stored setting, not a baked-in constant", () => {
    assert.equal(getRetentionN(db), 30); // settled default
    setRetentionN(db, 12);
    assert.equal(getRetentionN(db), 12);
    assert.throws(() => setRetentionN(db, 0), /between 1 and 500/);
    assert.throws(() => setRetentionN(db, 9999), /between 1 and 500/);
    assert.equal(getRetentionN(db), 12); // rejected values change nothing
    setRetentionN(db, 30);
  });

  test("N bounds the resident portion of the log", () => {
    const id = createDeck(db, "Retention slice");
    const stmt = db.prepare(
      "INSERT INTO decision_log (deck_id, revision, kind, action, card_name, rationale) VALUES (?, 0, 'accept', 'add', ?, 'r')",
    );
    for (let i = 0; i < 20; i++) stmt.run(id, `Sol Ring`);

    const count = (n: number) =>
      (assembleContext(db, id, n).segments.find((s) => s.key === "log")!.text.match(/^- accepted/gm) ??
        []).length;
    assert.equal(count(5), 5);
    assert.equal(count(20), 20);
    assert.match(assembleContext(db, id, 5).segments.find((s) => s.key === "log")!.text, /last 5/);
  });
});
