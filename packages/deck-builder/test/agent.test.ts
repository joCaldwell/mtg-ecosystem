import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { openDb } from "../src/db.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { addCard, createDeck, createSlot, createTag, updateCard } from "../src/deck/service.ts";
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
