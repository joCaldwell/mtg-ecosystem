import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { openDb } from "../src/db.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { addCard, createDeck, createSlot, createTag, getDeck, updateCard } from "../src/deck/service.ts";
import { executeTool } from "../src/agent/tools.ts";
import { listProposals, rejectItem, createProposal, acceptItem } from "../src/deck/proposals.ts";
import { updateBrief, setEngine, listBriefEdits, acceptBriefEdit, rejectBriefEdit, getBrief, proposeBriefEdit } from "../src/deck/brief.ts";
import { assembleContext } from "../src/agent/context.ts";
import { extractCardRefs, lintOutput } from "../src/agent/lint.ts";
import { runTurn, AgentError, getChatHistory } from "../src/agent/agent.ts";
import type { ChatMessage, ChatResponse, ChatTransport } from "../src/agent/llm.ts";

const db = openDb(":memory:");
ingestCards(db, FIXTURES);

function scripted(responses: Array<Partial<ChatResponse> & { message: ChatMessage }>): {
  transport: ChatTransport;
  requests: Array<{ messages: ChatMessage[] }>;
} {
  const queue = [...responses];
  const requests: Array<{ messages: ChatMessage[] }> = [];
  return {
    requests,
    transport: async (req) => {
      requests.push({ messages: [...req.messages] });
      const next = queue.shift();
      if (!next) throw new Error("Mock transport exhausted");
      return { finish_reason: "stop", ...next };
    },
  };
}

const text = (t: string): ChatMessage => ({ role: "assistant", content: t });
const toolCall = (name: string, args: object, id = "call_1"): ChatMessage => ({
  role: "assistant",
  content: null,
  tool_calls: [{ id, type: "function", function: { name, arguments: JSON.stringify(args) } }],
});

describe("lint (spec §6.4)", () => {
  test("extracts [[refs]] and resolves exactly, including face names", () => {
    assert.deepEqual(extractCardRefs("Add [[Sol Ring]] and [[Witch-Blessed Meadow]]."), [
      "Sol Ring",
      "Witch-Blessed Meadow",
    ]);
    assert.ok(lintOutput(db, "Run [[Sol Ring]] and [[witch-blessed meadow]].").ok);
  });
  test("never fuzzy-matches: near-miss fails with suggestions", () => {
    const r = lintOutput(db, "I like [[Seedborn Sage]] here.");
    assert.equal(r.ok, false);
    assert.equal(r.failures[0].name, "Seedborn Sage");
    assert.ok(r.failures[0].suggestions.includes("Seedborn Muse"));
  });
});

describe("context assembly (spec §10)", () => {
  test("segments present, decklist grouped by slot, owned NEVER included", () => {
    const id = createDeck(db, "Ctx deck");
    const ramp = createSlot(db, id, "Ramp", 8, 12);
    const t = createTag(db, id, "wincon");
    addCard(db, id, "id-atraxa", { role: "commander" });
    addCard(db, id, "id-solring", { slotId: ramp });
    updateCard(db, id, "id-solring", { owned: true, tagIds: [t] });
    updateBrief(db, id, { thesis: "Proliferate value grind", constraints_md: "No infinite combos" });
    setEngine(db, id, "Counters", "proliferate loop", [{ oracle_id: "id-atraxa" }]);

    const ctx = assembleContext(db, id, 30);
    assert.match(ctx.system, /CARD KNOWLEDGE CONTRACT/);
    assert.match(ctx.system, /Proliferate value grind/);
    assert.match(ctx.system, /## Ramp \(1/);
    assert.match(ctx.system, /\[\[Sol Ring\]\]/);
    assert.match(ctx.system, /\{T\}: Add \{C\}\{C\}\./); // oracle text present
    assert.match(ctx.system, /tags: wincon/);
    assert.match(ctx.system, /Counters: proliferate loop/);
    assert.match(ctx.system, /Cards: 2\/100/);
    // The owned flag must be structurally absent (spec §3)
    assert.ok(!/owned/i.test(ctx.system));
    assert.ok(!/owned/i.test(ctx.tailRestate));
  });
  test("tail restate covers count, slot deltas, hard filter names", () => {
    const id = createDeck(db, "Tail deck");
    createSlot(db, id, "Interaction", 8, 12);
    createProposal(db, id, [{ action: "add", oracle_id: "id-ball", rationale: "r" }]);
    rejectItem(
      db,
      (listProposals(db, id, "open")[0].items[0] as any).id,
      "hard_filter",
      "not this deck",
    );
    const ctx = assembleContext(db, id, 30);
    assert.match(ctx.tailRestate, /100 needed/);
    assert.match(ctx.tailRestate, /Interaction -8/);
    assert.match(ctx.tailRestate, /Ball Lightning/);
    // reasons live in the log, not the tail (spec §10)
    assert.ok(!ctx.tailRestate.includes("not this deck"));
    assert.match(ctx.system, /\[\[Ball Lightning\]\]: "not this deck"/);
  });
  test("retention N bounds the recent-decisions section", () => {
    const id = createDeck(db, "Retention deck");
    for (let i = 0; i < 5; i++) {
      addCard(db, id, "id-forest");
    }
    const ctx = assembleContext(db, id, 2);
    const section = ctx.system.split("## Recent decisions")[1];
    assert.ok(section);
  });
});

describe("agent loop", () => {
  test("hallucinated name never reaches the caller; lint bounce corrects it", async () => {
    const id = createDeck(db, "Bounce deck");
    const { transport, requests } = scripted([
      { message: text("You should add [[Seedborn Sage]] for untaps.") },
      { message: text("You should add [[Seedborn Muse]] for untaps.") },
    ]);
    const result = await runTurn(db, id, "any untap payoffs?", transport, 30);
    assert.equal(result.reply, "You should add [[Seedborn Muse]] for untaps.");
    // Second request must contain the lint correction as a system message
    const last = requests[1].messages.at(-1)!;
    assert.equal(last.role, "system");
    assert.match(last.content!, /Seedborn Sage.*does not resolve/);
    assert.match(last.content!, /Seedborn Muse/); // did-you-mean offered
  });

  test("persistent hallucination errors out — nothing shown", async () => {
    const id = createDeck(db, "Persistent deck");
    const bad = { message: text("Try [[Totally Fake Card]].") };
    const { transport } = scripted([bad, bad, bad]);
    await assert.rejects(() => runTurn(db, id, "ideas?", transport, 30), AgentError);
  });

  test("search → propose flow creates an agent proposal; deck untouched", async () => {
    const id = createDeck(db, "Tool deck");
    addCard(db, id, "id-teferi", { role: "commander" }); // mono-U identity
    const { transport, requests } = scripted([
      { message: toolCall("search_cards", { query: "t:instant o:counter" }) },
      {
        message: toolCall("propose_changes", {
          note: "protection",
          items: [
            { action: "add", oracle_id: "id-counterspell", rationale: "cheapest hard counter" },
          ],
        }),
      },
      { message: text("Proposed [[Counterspell]] — cheap protection for the engine.") },
    ]);
    const result = await runTurn(db, id, "add protection", transport, 30);
    assert.equal(result.mutatedState, true);
    assert.match(result.reply, /Counterspell/);

    // Search result was fed back and pre-filtered by identity
    const toolMsg = requests[1].messages.at(-1)!;
    assert.equal(toolMsg.role, "tool");
    assert.match(toolMsg.content!, /\[\[Counterspell\]\]/);
    assert.ok(!toolMsg.content!.includes("Ball Lightning")); // red never comes back

    // Proposal exists with source=agent, deck itself unchanged (no direct edits)
    const [p] = listProposals(db, id, "open");
    assert.equal(p.source, "agent");
    assert.equal(p.items[0].card_name, "Counterspell");
    const { getDeck } = await import("../src/deck/service.ts");
    assert.equal(getDeck(db, id).cards.length, 1); // still just the commander
  });

  // The proposal has to be findable from the transcript, not just from the
  // decision log: the owner reads the reply and rules on it in place.
  test("the turn's proposal is linked to its chat message and hydrated on read", async () => {
    const id = createDeck(db, "Inline proposal deck");
    addCard(db, id, "id-teferi", { role: "commander" });
    const { transport } = scripted([
      {
        message: toolCall("propose_changes", {
          note: "protection",
          items: [
            { action: "add", oracle_id: "id-counterspell", rationale: "cheapest hard counter" },
          ],
        }),
      },
      { message: text("Proposed [[Counterspell]].") },
    ]);
    await runTurn(db, id, "add protection", transport, 30);

    const history = getChatHistory(db, id);
    const linked = history.filter((m: any) => m.proposal_id != null);
    assert.equal(linked.length, 1);
    assert.equal(linked[0].role, "tool");

    // Hydrated from the proposal tables, so the card is renderable without a
    // second fetch — and the tool_call_id ties it back to the assistant turn.
    const p = (linked[0] as any).proposal;
    assert.equal(p.source, "agent");
    assert.equal(p.items.length, 1);
    assert.equal(p.items[0].card_name, "Counterspell");
    assert.equal(p.items[0].status, "pending");
    const assistant = history.find((m) => m.role === "assistant" && m.tool_calls?.length)!;
    assert.equal((linked[0] as any).tool_call_id, assistant.tool_calls![0].id);

    // Status is read live, so a ruling shows in the transcript rather than
    // freezing at whatever it was when the turn ran.
    acceptItem(db, p.items[0].id);
    const after = getChatHistory(db, id).find((m: any) => m.proposal_id != null) as any;
    assert.equal(after.proposal.items[0].status, "accepted");
    assert.equal(after.proposal.status, "resolved");
  });

  // The owner hands the agent a finding as a reference, not as pasted text —
  // so the agent has to read the recorded run before it can answer.
  test("get_audit resolves an audit#run/key reference to the recorded finding", async () => {
    const { runAudit } = await import("../src/deck/audit.ts");
    const id = createDeck(db, "Audit ref deck");
    addCard(db, id, "id-teferi", { role: "commander" });
    const run = runAudit(db, id, "why can't this win?", {
      summary: "No closer.",
      findings: [
        {
          key: "reasoning:no-win-path",
          severity: "warn",
          title: "No way to actually win",
          detail: "Nothing in the list converts the mana into damage.",
        },
      ],
      dismissed: [],
      dropped: 0,
    });

    const { transport, requests } = scripted([
      {
        message: toolCall("get_audit", {
          run_id: run.run_id,
          finding_key: "reasoning:no-win-path",
        }),
      },
      { message: text("Agreed — [[Teferi, Temporal Archmage]] stalls but never closes.") },
    ]);
    await runTurn(db, id, `what about audit#${run.run_id}/reasoning:no-win-path ?`, transport, 30);

    const toolMsg = requests[1].messages.at(-1)!;
    assert.equal(toolMsg.role, "tool");
    assert.match(toolMsg.content!, /No way to actually win/);
    assert.match(toolMsg.content!, /converts the mana into damage/);
    assert.match(toolMsg.content!, /judgement/); // labelled as §8.2, not authoritative
    assert.match(toolMsg.content!, /why can't this win\?/); // the run's own instructions
  });

  test("get_audit with no arguments returns the whole latest run", async () => {
    const { runAudit } = await import("../src/deck/audit.ts");
    const id = createDeck(db, "Audit whole-run deck");
    runAudit(db, id, "", {
      summary: "Two cards short and no interaction.",
      findings: [
        { key: "reasoning:no-interaction", severity: "error", title: "No interaction", detail: "None." },
      ],
      dismissed: [],
      dropped: 0,
    });
    const { transport, requests } = scripted([
      { message: toolCall("get_audit", {}) },
      { message: text("The audit is right about interaction.") },
    ]);
    await runTurn(db, id, "what did the audit say?", transport, 30);

    const toolMsg = requests[1].messages.at(-1)!.content!;
    assert.match(toolMsg, /Two cards short and no interaction/); // the summary
    assert.match(toolMsg, /\[card_count\]/); // the deterministic half too
    assert.match(toolMsg, /\[reasoning:no-interaction\]/);
  });

  test("get_audit refuses to invent a finding it cannot find", async () => {
    const id = createDeck(db, "Audit missing ref deck");
    const { transport, requests } = scripted([
      { message: toolCall("get_audit", { finding_key: "reasoning:made-up" }) },
      { message: text("I can't find that finding — what did it say?") },
    ]);
    await runTurn(db, id, "explain audit/reasoning:made-up", transport, 30);
    assert.match(requests[1].messages.at(-1)!.content!, /No audit finding with key/);
  });

  // Regression: a deck with no slots (the default) must still be proposable.
  // An unmatched slot_name used to abort the whole proposal, which read to the
  // agent as "proposals require a slot" and stalled it into asking for one.
  test("an unknown slot name downgrades the item to unslotted, it does not fail", async () => {
    const id = createDeck(db, "Slotless deck");
    addCard(db, id, "id-teferi", { role: "commander" });
    const { transport, requests } = scripted([
      {
        message: toolCall("propose_changes", {
          note: "draw",
          items: [
            {
              action: "add",
              oracle_id: "id-counterspell",
              slot_name: "Draw",
              rationale: "cheapest hard counter",
            },
          ],
        }),
      },
      { message: text("Proposed [[Counterspell]].") },
    ]);
    await runTurn(db, id, "add draw", transport, 30);

    const toolMsg = requests[1].messages.at(-1)!;
    assert.doesNotMatch(toolMsg.content!, /Error/);
    assert.match(toolMsg.content!, /No slot named 'Draw'/);
    assert.match(toolMsg.content!, /slots are optional/);

    const [p] = listProposals(db, id, "open");
    assert.equal(p.items.length, 1);
    assert.equal(p.items[0].card_name, "Counterspell");
    assert.equal(p.items[0].slot_id, null);

    // ...and it accepts cleanly, landing unslotted.
    acceptItem(db, (p.items[0] as any).id);
    const { getDeck } = await import("../src/deck/service.ts");
    const state = getDeck(db, id);
    assert.equal(state.computed.unslotted_count, 2); // commander + the accepted add
    assert.equal(state.cards.find((c) => c.oracle_id === "id-counterspell")!.slot_id, null);
  });

  test("a fabricated oracle_id physically cannot enter the deck (spec §6.2)", async () => {
    const id = createDeck(db, "Fabricated deck");
    const { transport, requests } = scripted([
      {
        message: toolCall("propose_changes", {
          items: [{ action: "add", oracle_id: "made-up-uuid-1234", rationale: "trust me" }],
        }),
      },
      { message: text("That card is not in the database, so I could not propose it.") },
    ]);
    await runTurn(db, id, "add that card", transport, 30);
    const toolMsg = requests[1].messages.at(-1)!;
    assert.match(toolMsg.content!, /Error: Unknown oracle_id/);
    assert.equal(listProposals(db, id).length, 0);
  });

  test("hard-filtered cards are unfindable and unproposable by the agent", async () => {
    const id = createDeck(db, "Filtered deck");
    createProposal(db, id, [{ action: "add", oracle_id: "id-solring", rationale: "r" }]);
    rejectItem(db, (listProposals(db, id, "open")[0].items[0] as any).id, "hard_filter", "own zero copies");

    const { transport, requests } = scripted([
      { message: toolCall("search_cards", { query: '!"sol ring"' }) },
      {
        message: toolCall("propose_changes", {
          items: [{ action: "add", oracle_id: "id-solring", rationale: "staple" }],
        }),
      },
      { message: text("Understood — [[Sol Ring]] is filtered for this deck.") },
    ]);
    await runTurn(db, id, "why no sol ring?", transport, 30);
    assert.match(requests[1].messages.at(-1)!.content!, /No results/);
    assert.match(requests[2].messages.at(-1)!.content!, /hard-filtered/);
    assert.equal(listProposals(db, id, "open").length, 0);
  });

  test("transcript persists across turns (one chat per deck)", async () => {
    const id = createDeck(db, "History deck");
    const t1 = scripted([{ message: text("Noted.") }]);
    await runTurn(db, id, "remember: budget is $200", t1.transport, 30);
    const t2 = scripted([{ message: text("Still noted.") }]);
    await runTurn(db, id, "still there?", t2.transport, 30);
    // Second turn's request must replay the first turn's messages
    const replayed = t2.requests[0].messages.map((m) => m.content ?? "");
    assert.ok(replayed.some((c) => c.includes("budget is $200")));
    assert.ok(replayed.some((c) => c === "Noted."));
    assert.equal(getChatHistory(db, id).length, 4);
  });
});

describe("slot tools (spec §4 — organization applies directly)", () => {
  /** A deck with a commander and five slottable cards, nothing filed yet. */
  function organizedDeck(name: string) {
    const id = createDeck(db, name);
    addCard(db, id, "id-atraxa", { role: "commander" });
    for (const oid of ["id-llanowar", "id-solring", "id-counterspell", "id-seedborn", "id-witch"])
      addCard(db, id, oid);
    return id;
  }
  const slotOf = (id: number, oracleId: string) =>
    getDeck(db, id).cards.find((c) => c.oracle_id === oracleId)!.slot_id;

  test("create_slot then a bulk move files many cards in one call", async () => {
    const id = organizedDeck("Filing deck");
    const before = getDeck(db, id);

    const { transport, requests } = scripted([
      {
        message: toolCall("create_slot", { name: "Ramp", target_min: 8, target_max: 12 }),
      },
      {
        message: toolCall(
          "move_cards",
          { moves: [{ slot_name: "ramp", cards: ["Llanowar Elves", "Sol Ring", "id-seedborn"] }] },
          "call_2",
        ),
      },
      { message: text("Filed [[Llanowar Elves]], [[Sol Ring]] and [[Seedborn Muse]] under ramp.") },
    ]);
    const turn = await runTurn(db, id, "organize the ramp", transport, 30);

    assert.match(requests[1].messages.at(-1)!.content!, /Slot 'Ramp' created/);
    const moveResult = requests[2].messages.at(-1)!.content!;
    assert.match(moveResult, /Moved 3 card\(s\)/);
    assert.match(moveResult, /Ramp: 3 \(target 8–12, under by 5\)/);
    assert.equal(turn.mutatedState, true);

    const after = getDeck(db, id);
    // Organization only: same cards, same count, same revision.
    assert.equal(after.cards.length, before.cards.length);
    assert.equal(after.deck.revision, before.deck.revision);
    assert.equal(after.computed.card_count, before.computed.card_count);
    const ramp = after.slots.find((s) => s.name === "Ramp")!;
    assert.deepEqual(
      after.cards.filter((c) => c.slot_id === ramp.id).map((c) => c.oracle_id).sort(),
      ["id-llanowar", "id-seedborn", "id-solring"],
    );
    assert.equal(after.computed.slot_deltas[0].delta, -5);
  });

  test("names resolve against the deck, face names included; oracle_ids also work", () => {
    const id = organizedDeck("Face name deck");
    createSlot(db, id, "Utility");
    const out = executeTool(db, id, "move_cards", {
      // The back face of a modal card, and a name the decklist prints in a
      // different case, both name the same deck card.
      moves: [{ slot_name: "utility", cards: ["witch-blessed meadow", "COUNTERSPELL"] }],
    });
    assert.equal(out.isError, false);
    const utility = getDeck(db, id).slots[0].id;
    assert.equal(slotOf(id, "id-witch"), utility);
    assert.equal(slotOf(id, "id-counterspell"), utility);
  });

  test("one bad reference rejects the whole call — no half-refiled deck", () => {
    const id = organizedDeck("Atomic deck");
    createSlot(db, id, "Interaction");
    const out = executeTool(db, id, "move_cards", {
      moves: [{ slot_name: "Interaction", cards: ["Counterspell", "Swords to Plowshares"] }],
    });
    assert.equal(out.isError, true);
    assert.equal(out.mutatedState, false);
    assert.match(out.result, /'Swords to Plowshares' is not a card in this deck/);
    assert.equal(slotOf(id, "id-counterspell"), null);
  });

  test("an unknown slot fails loudly here and names the slots that exist", () => {
    const id = organizedDeck("Unknown slot deck");
    createSlot(db, id, "Ramp");
    const out = executeTool(db, id, "move_cards", {
      moves: [{ slot_name: "Draw", cards: ["Sol Ring"] }],
    });
    assert.equal(out.isError, true);
    assert.match(out.result, /No slot named 'Draw'/);
    assert.match(out.result, /Existing slots: Ramp/);
    assert.equal(slotOf(id, "id-solring"), null);
  });

  test("command-zone cards cannot be filed into a slot", () => {
    const id = organizedDeck("Commander slot deck");
    createSlot(db, id, "Ramp");
    const out = executeTool(db, id, "move_cards", {
      moves: [{ slot_name: "Ramp", cards: ["Atraxa, Praetors' Voice"] }],
    });
    assert.equal(out.isError, true);
    assert.match(out.result, /command zone/);
    assert.equal(slotOf(id, "id-atraxa"), null);
  });

  test("omitting slot_name unslots; a card cannot be sent to two slots at once", () => {
    const id = organizedDeck("Unslot deck");
    const ramp = createSlot(db, id, "Ramp");
    createSlot(db, id, "Interaction");
    executeTool(db, id, "move_cards", { moves: [{ slot_name: "Ramp", cards: ["Sol Ring"] }] });
    assert.equal(slotOf(id, "id-solring"), ramp);

    const conflict = executeTool(db, id, "move_cards", {
      moves: [
        { slot_name: "Ramp", cards: ["Counterspell"] },
        { slot_name: "Interaction", cards: ["Counterspell"] },
      ],
    });
    assert.equal(conflict.isError, true);
    assert.match(conflict.result, /sent to both/);

    const out = executeTool(db, id, "move_cards", { moves: [{ cards: ["Sol Ring"] }] });
    assert.equal(out.isError, false);
    assert.equal(slotOf(id, "id-solring"), null);
  });

  test("update_slot renames and retargets; an explicit null clears a target", () => {
    const id = organizedDeck("Retarget deck");
    createSlot(db, id, "Ramp", 8, 12);

    assert.equal(executeTool(db, id, "update_slot", { slot_name: "Ramp", new_name: "Acceleration", target_min: 10 }).isError, false);
    let slot = getDeck(db, id).slots[0];
    assert.equal(slot.name, "Acceleration");
    assert.equal(slot.target_min, 10);
    assert.equal(slot.target_max, 12);

    executeTool(db, id, "update_slot", { slot_name: "acceleration", target_max: null });
    slot = getDeck(db, id).slots[0];
    assert.equal(slot.target_min, 10);
    assert.equal(slot.target_max, null);

    // Absent is not null: leaving both out changes nothing and says so.
    const noop = executeTool(db, id, "update_slot", { slot_name: "Acceleration" });
    assert.equal(noop.isError, true);
    assert.match(noop.result, /Nothing to change/);
  });

  test("deleting a slot unslots its cards without cutting them, and lists them", () => {
    const id = organizedDeck("Delete slot deck");
    createSlot(db, id, "Ramp");
    executeTool(db, id, "move_cards", {
      moves: [{ slot_name: "Ramp", cards: ["Sol Ring", "Llanowar Elves"] }],
    });
    const before = getDeck(db, id).computed.card_count;

    const out = executeTool(db, id, "delete_slot", { slot_name: "Ramp" });
    assert.equal(out.isError, false);
    assert.match(out.result, /\[\[Sol Ring\]\]/);
    assert.match(out.result, /\[\[Llanowar Elves\]\]/);

    const after = getDeck(db, id);
    assert.equal(after.slots.length, 0);
    assert.equal(after.computed.card_count, before);
    assert.equal(slotOf(id, "id-solring"), null);
  });

  test("slot names may not be card types, and duplicates are refused", () => {
    const id = organizedDeck("Naming deck");
    const typed = executeTool(db, id, "create_slot", { name: "Lands" });
    assert.equal(typed.isError, true);
    assert.match(typed.result, /card type/);

    assert.equal(executeTool(db, id, "create_slot", { name: "Ramp" }).isError, false);
    const dupe = executeTool(db, id, "create_slot", { name: "ramp" });
    assert.equal(dupe.isError, true);
    assert.match(dupe.result, /already exists/);
    assert.equal(getDeck(db, id).slots.length, 1);
  });
});

describe("brief service", () => {
  test("engine edit proposals apply through the gate", () => {
    const id = createDeck(db, "Brief deck");
    proposeBriefEdit(db, id, "thesis", { content: "Untap everything, win with mana" }, "owner keeps accepting untap cards");
    proposeBriefEdit(db, id, "engine_set", {
      engine_name: "Untap loop",
      description: "untap + payoff",
      pieces: [{ oracle_id: "id-seedborn" }],
    }, "recurring theme in accepted proposals");

    const pending = listBriefEdits(db, id, "pending");
    assert.equal(pending.length, 2);
    acceptBriefEdit(db, id, pending.find((e) => e.kind === "thesis")!.id);
    rejectBriefEdit(db, id, pending.find((e) => e.kind === "engine_set")!.id, "soft", "not yet");

    const brief = getBrief(db, id);
    assert.equal(brief.thesis, "Untap everything, win with mana");
    assert.equal(brief.engines.length, 0);
    assert.equal(listBriefEdits(db, id, "pending").length, 0);
  });
  test("accepting an engine_set edit creates the engine", () => {
    // Regression: acceptBriefEdit wraps the apply in a transaction and
    // setEngine opens its own — this only works because withTransaction
    // nests (plain BEGIN would throw here).
    const id = createDeck(db, "Engine accept deck");
    proposeBriefEdit(db, id, "engine_set", {
      engine_name: "Untap loop",
      description: "untap + payoff",
      pieces: [{ oracle_id: "id-seedborn" }],
    }, "recurring theme");
    const [edit] = listBriefEdits(db, id, "pending");
    acceptBriefEdit(db, id, edit.id);

    const brief = getBrief(db, id);
    assert.equal(brief.engines.length, 1);
    assert.equal(brief.engines[0].name, "Untap loop");
    assert.equal(listBriefEdits(db, id, "pending").length, 0);
  });
  test("engine pieces track deck membership", () => {
    const id = createDeck(db, "Engine deck");
    addCard(db, id, "id-seedborn");
    setEngine(db, id, "Untap", "", [{ oracle_id: "id-seedborn" }, { oracle_id: "id-counterspell" }]);
    const [engine] = getBrief(db, id).engines;
    const byName = Object.fromEntries(engine.pieces.map((p) => [p.name, p.in_deck]));
    assert.equal(byName["Seedborn Muse"], true);
    assert.equal(byName["Counterspell"], false);
  });
});
