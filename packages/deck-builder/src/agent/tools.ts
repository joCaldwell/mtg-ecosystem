// Agent tools (spec §6.2/§6.3): mutations take oracle_id, never names —
// the only way to add a card is to have found it in a search result first.
// Searches are pre-filtered by deck color identity and exclude hard filters.

import type { DatabaseSync } from "node:sqlite";
import { search, SearchError } from "../search/index.ts";
import { createProposal } from "../deck/proposals.ts";
import { getCardHistory } from "../deck/proposals.ts";
import { proposeBriefEdit } from "../deck/brief.ts";
import { ServiceError } from "../deck/service.ts";
import {
  getAuditRun,
  listAuditRuns,
  lookupFinding,
  type AuditRun,
  type FoundFinding,
} from "../deck/audit.ts";
import type { ToolDef } from "./llm.ts";

export const TOOL_DEFS: ToolDef[] = [
  {
    type: "function",
    function: {
      name: "search_cards",
      description:
        "Search the card database with Scryfall-style syntax (t:, o:, cmc/mv, c:, id:, pow/tou, is:commander, slot:, tag:, quoted phrases, OR, negation with -). Results are pre-filtered to the deck's color identity and exclude hard-filtered cards. This is the ONLY source of card facts outside the decklist — search before referencing any card not in the deck.",
      parameters: {
        type: "object",
        properties: {
          query: { type: "string", description: "Search query, e.g. 't:creature o:\"untap\" mv<=3'" },
          limit: { type: "integer", description: "Max results (default 10, max 25)" },
        },
        required: ["query"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "propose_changes",
      description:
        "Propose deck changes for the owner to rule on. 3–5 items maximum — rank your best ideas. oracle_id values MUST come from this conversation's search results or the decklist. Items sharing a group_id are accepted/rejected as a unit (use for swaps). Slots are optional — never withhold a proposal for want of one.",
      parameters: {
        type: "object",
        properties: {
          note: { type: "string", description: "One-line summary of the changeset" },
          items: {
            type: "array",
            items: {
              type: "object",
              properties: {
                action: { type: "string", enum: ["add", "cut"] },
                oracle_id: { type: "string" },
                slot_name: {
                  type: "string",
                  description:
                    "Optional. Target slot for adds, if the deck defines slots and one clearly fits. Omit it otherwise — the card is added unslotted, which is always valid.",
                },
                rationale: { type: "string", description: "Why — required, this is what the owner rules on" },
                group_id: { type: "string", description: "Same value on items that must be ruled on together" },
              },
              required: ["action", "oracle_id", "rationale"],
            },
          },
        },
        required: ["items"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "propose_brief_edit",
      description:
        "Propose replacing the brief's thesis or constraints text. Goes to the owner for approval. Use when rulings show the written brief is stale (e.g. repeated thesis_change rejections).",
      parameters: {
        type: "object",
        properties: {
          section: { type: "string", enum: ["thesis", "constraints"] },
          content: { type: "string", description: "Full replacement text for the section" },
          rationale: { type: "string" },
        },
        required: ["section", "content", "rationale"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "propose_engine_edit",
      description:
        "Propose creating/updating a named engine (action 'set') or removing one (action 'remove'). Engine pieces are oracle_ids from search results or the decklist. Goes to the owner for approval.",
      parameters: {
        type: "object",
        properties: {
          action: { type: "string", enum: ["set", "remove"] },
          name: { type: "string", description: "Engine name" },
          description: { type: "string" },
          piece_oracle_ids: { type: "array", items: { type: "string" } },
          rationale: { type: "string" },
        },
        required: ["action", "name", "rationale"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "get_audit",
      description:
        "Read a recorded audit run. Call this whenever the owner references a finding as audit#<run_id>/<finding_key> — that token is a pointer, not the finding, and you must read the finding before answering. With no arguments it returns the most recent run in full (summary plus every finding), which is how you get the picture surrounding a single referenced finding.",
      parameters: {
        type: "object",
        properties: {
          run_id: { type: "integer", description: "Audit run id; omit for the most recent run" },
          finding_key: {
            type: "string",
            description:
              "Return just this finding (the part after the slash in audit#12/reasoning:no-win-path). Omit for the whole run.",
          },
        },
      },
    },
  },
  {
    type: "function",
    function: {
      name: "get_card_history",
      description:
        "Get the full decision history for one card in this deck (accepts, rejections with types and reasons, undos). Required before re-proposing anything previously rejected.",
      parameters: {
        type: "object",
        properties: {
          oracle_id: { type: "string" },
        },
        required: ["oracle_id"],
      },
    },
  },
];

export interface ToolOutcome {
  result: string;
  isError: boolean;
  mutatedState: boolean;
  /** Set by propose_changes. The turn records it on the persisted tool message
   *  so the chat can render the proposal itself, not just a line saying one
   *  happened — the owner rules from the transcript without leaving it. */
  proposalId?: number;
}

// ---------- audit formatting ----------

function currentRevision(db: DatabaseSync, deckId: number): number {
  return (db.prepare("SELECT revision FROM decks WHERE id = ?").get(deckId) as { revision: number })
    .revision;
}

// A run is a snapshot. Saying so is load-bearing: without it the model treats
// a finding from four accepted proposals ago as a description of the deck
// that is in front of it.
function runHeader(run: AuditRun, revision: number): string {
  const age = run.revision === revision
    ? "the deck's current revision"
    : `revision ${run.revision}; the deck is now at ${revision}, so its deterministic findings may already be resolved`;
  const status =
    run.status === "running"
      ? " (still running — the reasoning half is not in yet)"
      : run.status === "error"
        ? ` (failed: ${run.error})`
        : "";
  return `Audit run #${run.id}${status}, ${run.created_at}, taken at ${age}.${
    run.instructions ? `\nThe owner's focus for this run: "${run.instructions}"` : ""
  }`;
}

function findingLine(f: { key: string; severity: string; title: string; detail: string }): string {
  return `- [${f.key}] ${f.severity.toUpperCase()}: ${f.title}\n  ${f.detail}`;
}

function formatFinding(hit: FoundFinding, revision: number): string {
  const origin = hit.run
    ? runHeader(hit.run, revision)
    : "Live deterministic check against the deck as it stands right now.";
  const half =
    hit.source === "reasoning"
      ? "This is a reasoning finding (§8.2) — a judgement, and one you may disagree with if the oracle text supports you."
      : "This is a deterministic finding (§8.1) — computed in SQL and authoritative. Do not re-derive it.";
  const dismissed = hit.dismissal
    ? `\nThe owner has DISMISSED this finding (${hit.dismissal.type}): "${hit.dismissal.reason}". Take that ruling seriously before re-arguing it.`
    : "";
  return `${origin}\n${half}\n\n${findingLine(hit.finding)}${dismissed}`;
}

function formatRun(run: AuditRun, revision: number): string {
  const parts = [runHeader(run, revision)];
  parts.push(
    `\n## Deterministic findings (${run.findings.length})\n${
      run.findings.map(findingLine).join("\n") || "(none)"
    }`,
  );
  if (run.reasoning?.error) {
    parts.push(`\n## Reasoning pass\nUnavailable for this run: ${run.reasoning.error}`);
  } else if (run.reasoning) {
    const r = run.reasoning;
    parts.push(
      `\n## Reasoning pass\n${r.summary || "(no summary)"}\n\n${
        r.findings.map(findingLine).join("\n") || "(no findings)"
      }`,
    );
    if (r.dismissed.length)
      parts.push(
        `\n## Reasoning findings the owner dismissed\n${r.dismissed
          .map((f) => `${findingLine(f)}\n  dismissed (${f.dismissal.type}): "${f.dismissal.reason}"`)
          .join("\n")}`,
      );
  }
  return parts.join("\n");
}

export function executeTool(
  db: DatabaseSync,
  deckId: number,
  name: string,
  args: any,
): ToolOutcome {
  try {
    switch (name) {
      case "search_cards": {
        const limit = Math.min(Number(args.limit ?? 10), 25);
        const results = search(db, String(args.query ?? ""), {
          deckId,
          limit,
          excludeHardFilters: true,
        });
        if (!results.length)
          return {
            result:
              "No results (within this deck's color identity, hard filters excluded). Broaden the query or check the syntax.",
            isError: false,
            mutatedState: false,
          };
        const lines = results.map((c) => {
          const pt =
            c.power != null && c.toughness != null
              ? ` ${c.power}/${c.toughness}`
              : c.loyalty != null
                ? ` [${c.loyalty}]`
                : "";
          return `[[${c.name}]] ${c.mana_cost ?? ""} — ${c.type_line}${pt} (id: ${c.color_identity || "C"})\noracle_id: ${c.oracle_id}\n${c.oracle_text}`;
        });
        return {
          result: `${results.length} result(s):\n\n${lines.join("\n\n")}`,
          isError: false,
          mutatedState: false,
        };
      }

      case "propose_changes": {
        // Slots are an optional organizational overlay (spec §4) — a proposal
        // never requires one. An unknown slot name downgrades that item to
        // unslotted instead of failing the whole proposal; the owner can slot
        // it after accepting. Failing here taught the agent that a deck with no
        // slots couldn't be proposed to at all.
        const unknownSlots: string[] = [];
        const items = (args.items ?? []).map((i: any) => {
          let slotId: number | null = null;
          if (i.slot_name) {
            const slot = db
              .prepare("SELECT id FROM slots WHERE deck_id = ? AND name = ? COLLATE NOCASE")
              .get(deckId, String(i.slot_name)) as { id: number } | undefined;
            if (slot) slotId = slot.id;
            else unknownSlots.push(String(i.slot_name));
          }
          return {
            action: i.action,
            oracle_id: String(i.oracle_id),
            slot_id: slotId,
            rationale: String(i.rationale ?? ""),
            group_id: i.group_id ? String(i.group_id) : null,
          };
        });
        const id = createProposal(db, deckId, items, {
          source: "agent",
          note: String(args.note ?? ""),
        });
        const slotNote = unknownSlots.length
          ? ` No slot named ${[...new Set(unknownSlots)].map((s) => `'${s}'`).join(", ")} exists in this deck, so those items were left unslotted — slots are optional and the owner can slot them later.`
          : "";
        return {
          result: `Proposal #${id} created with ${items.length} item(s).${slotNote} The owner will rule on it — do not assume acceptance.`,
          isError: false,
          mutatedState: true,
          proposalId: id,
        };
      }

      case "propose_brief_edit": {
        const id = proposeBriefEdit(
          db,
          deckId,
          args.section === "constraints" ? "constraints" : "thesis",
          { content: String(args.content ?? "") },
          String(args.rationale ?? ""),
        );
        return {
          result: `Brief edit #${id} proposed (${args.section}). Awaiting the owner's ruling.`,
          isError: false,
          mutatedState: true,
        };
      }

      case "propose_engine_edit": {
        const kind = args.action === "remove" ? "engine_remove" : "engine_set";
        const id = proposeBriefEdit(
          db,
          deckId,
          kind,
          {
            engine_name: String(args.name ?? ""),
            description: String(args.description ?? ""),
            pieces: (args.piece_oracle_ids ?? []).map((o: string) => ({ oracle_id: o })),
          },
          String(args.rationale ?? ""),
        );
        return {
          result: `Engine edit #${id} proposed (${kind}). Awaiting the owner's ruling.`,
          isError: false,
          mutatedState: true,
        };
      }

      case "get_audit": {
        const revision = currentRevision(db, deckId);
        const runId = args.run_id != null ? Number(args.run_id) : null;
        const key = args.finding_key ? String(args.finding_key) : null;

        if (key) {
          const hit = lookupFinding(db, deckId, key, runId);
          if (!hit)
            return {
              result: `No audit finding with key '${key}' in any retained run. It may have aged out of the last ${listAuditRuns(db, deckId).length} run(s) — ask the owner what the finding said rather than guessing.`,
              isError: true,
              mutatedState: false,
            };
          return { result: formatFinding(hit, revision), isError: false, mutatedState: false };
        }

        const run = runId != null ? getAuditRun(db, deckId, runId) : (listAuditRuns(db, deckId)[0] ?? null);
        if (!run)
          return {
            result: runId != null
              ? `No audit run #${runId} for this deck.`
              : "No audit has been run on this deck yet.",
            isError: true,
            mutatedState: false,
          };
        return { result: formatRun(run, revision), isError: false, mutatedState: false };
      }

      case "get_card_history": {
        const history = getCardHistory(db, deckId, String(args.oracle_id ?? "")) as any[];
        if (!history.length)
          return { result: "No decisions recorded for this card.", isError: false, mutatedState: false };
        const lines = history.map((e) => {
          if (e.kind === "reject")
            return `${e.ts}: rejected ${e.action} (${e.rejection_type}): "${e.rejection_reason}"`;
          return `${e.ts}: ${e.kind} ${e.action ?? ""}${e.rationale ? ` — ${e.rationale}` : ""}`;
        });
        return { result: lines.join("\n"), isError: false, mutatedState: false };
      }

      default:
        return { result: `Unknown tool: ${name}`, isError: true, mutatedState: false };
    }
  } catch (e: any) {
    if (e instanceof SearchError || e instanceof ServiceError) {
      return { result: `Error: ${e.message}`, isError: true, mutatedState: false };
    }
    throw e;
  }
}
