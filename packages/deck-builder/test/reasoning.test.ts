import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { openDb } from "../src/db.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { addCard, createDeck } from "../src/deck/service.ts";
import { updateBrief } from "../src/deck/brief.ts";
import {
  activeDismissals,
  dismissFinding,
  promoteFinding,
  runAudit,
} from "../src/deck/audit.ts";
import { listProposals } from "../src/deck/proposals.ts";
import { runReasoningPass } from "../src/agent/reasoning.ts";
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

function seedDeck(name: string): number {
  const id = createDeck(db, name);
  addCard(db, id, "id-teferi", { role: "commander" });
  addCard(db, id, "id-counterspell");
  addCard(db, id, "id-solring");
  updateBrief(db, id, { thesis: "Untap value; win with big mana." });
  return id;
}

describe("reasoning pass (spec §8.2)", () => {
  test("deterministic results are supplied as context, never re-derived", async () => {
    const id = seedDeck("Reason ctx");
    const { transport, prompts } = jsonTransport([JSON.stringify({ summary: "Fine.", findings: [] })]);
    const result = await runReasoningPass(db, id, "focus on the win path", transport, 30, new Map());
    assert.equal(result.summary, "Fine.");
    const system = prompts[0][0].content!;
    assert.match(system, /Cards: 3\/100/); // computed state present
    assert.match(system, /Untap value; win with big mana/); // brief present
    assert.match(system, /\[\[Counterspell\]\]/); // decklist present
    assert.match(system, /Never recount/);
    assert.match(prompts[0][1].content!, /focus on the win path/); // instructions flow through
  });

  test("findings naming nonexistent cards are dropped; in-deck cut targets resolve to oracle_id", async () => {
    const id = seedDeck("Reason validate");
    const { transport } = jsonTransport([
      JSON.stringify({
        summary: "Mixed quality output.",
        findings: [
          {
            slug: "solring-antisynergy",
            severity: "warn",
            title: "[[Sol Ring]] does not advance the thesis",
            detail: "Fast colorless mana, but the deck wants blue pips.",
            action: "cut",
            card_name: "Sol Ring",
          },
          {
            slug: "fake-card-claim",
            severity: "error",
            title: "You need [[Mana Drainn]]",
            detail: "hallucinated",
          },
          {
            slug: "cut-not-in-deck",
            severity: "warn",
            title: "Weak against aggro",
            detail: "Consider more early interaction.",
            action: "cut",
            card_name: "Llanowar Elves",
          },
        ],
      }),
    ]);
    const result = await runReasoningPass(db, id, "", transport, 30, new Map());
    assert.equal(result.dropped, 1); // the hallucinated one
    assert.equal(result.findings.length, 2);
    const cut = result.findings.find((f) => f.key === "reasoning:solring-antisynergy")!;
    assert.equal(cut.action, "cut");
    assert.equal(cut.oracle_id, "id-solring");
    // cut of a card not in deck survives but demoted to informational
    const demoted = result.findings.find((f) => f.key === "reasoning:cut-not-in-deck")!;
    assert.equal(demoted.action, undefined);
  });

  test("invalid JSON gets one retry with a correction", async () => {
    const id = seedDeck("Reason retry");
    const { transport, prompts } = jsonTransport([
      "Sure! Here are my thoughts about the deck...",
      JSON.stringify({ summary: "ok", findings: [] }),
    ]);
    const result = await runReasoningPass(db, id, "", transport, 30, new Map());
    assert.equal(result.error, undefined);
    assert.equal(prompts.length, 2);
    assert.match(prompts[1].at(-1)!.content!, /not valid JSON/);
  });

  test("dismissed reasoning findings are suppressed and prior slugs shown to the model", async () => {
    const id = seedDeck("Reason dismissed");
    const dismissals = new Map([
      ["reasoning:solring-antisynergy", { type: "playtest_finding", reason: "it performs fine" }],
    ]);
    const { transport, prompts } = jsonTransport([
      JSON.stringify({
        summary: "s",
        findings: [
          { slug: "solring-antisynergy", severity: "warn", title: "[[Sol Ring]] again", detail: "d" },
        ],
      }),
    ]);
    const result = await runReasoningPass(db, id, "", transport, 30, dismissals as any);
    assert.equal(result.findings.length, 0);
    assert.equal(result.dismissed.length, 1);
    assert.match(prompts[0][1].content!, /solring-antisynergy.*it performs fine/);
  });

  test("reasoning findings dismiss and promote through the same gate", async () => {
    const id = seedDeck("Reason gate");
    const reasoning = {
      summary: "s",
      findings: [
        {
          key: "reasoning:solring-cut",
          severity: "warn",
          title: "[[Sol Ring]] anti-synergy",
          detail: "d",
          action: "cut",
          oracle_id: "id-solring",
          card_name: "Sol Ring",
        },
        { key: "reasoning:thin-interaction", severity: "warn", title: "Interaction is thin", detail: "d" },
      ],
      dismissed: [],
      dropped: 0,
    };
    runAudit(db, id, "", reasoning); // records reasoning_json on the run

    // Promote the cut → audit-source proposal with the real oracle_id
    const pid = promoteFinding(db, id, "reasoning:solring-cut");
    const p = listProposals(db, id, "open").find((x) => x.id === pid)!;
    assert.equal(p.items[0].action, "cut");
    assert.equal(p.items[0].card_name, "Sol Ring");

    // Dismiss the informational one with a typed reason → suppressed next time
    dismissFinding(db, id, "reasoning:thin-interaction", "thesis_change", "deck is proactive on purpose");
    assert.ok(activeDismissals(db, id).has("reasoning:thin-interaction"));
  });
});
