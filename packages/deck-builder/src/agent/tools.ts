// Agent tools (spec §6.2/§6.3): mutations take oracle_id, never names —
// the only way to add a card is to have found it in a search result first.
// Searches are pre-filtered by deck color identity and exclude hard filters.

import type { DatabaseSync } from "node:sqlite";
import { search, SearchError } from "../search/index.ts";
import { createProposal } from "../deck/proposals.ts";
import { getCardHistory } from "../deck/proposals.ts";
import { proposeBriefEdit } from "../deck/brief.ts";
import { ServiceError } from "../deck/service.ts";
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
        "Propose deck changes for the owner to rule on. 3–5 items maximum — rank your best ideas. oracle_id values MUST come from this conversation's search results or the decklist. Items sharing a group_id are accepted/rejected as a unit (use for swaps).",
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
                slot_name: { type: "string", description: "Target slot for adds (must be an existing slot name)" },
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
        const items = (args.items ?? []).map((i: any) => {
          let slotId: number | null = null;
          if (i.slot_name) {
            const slot = db
              .prepare("SELECT id FROM slots WHERE deck_id = ? AND name = ? COLLATE NOCASE")
              .get(deckId, String(i.slot_name)) as { id: number } | undefined;
            if (!slot) throw new ServiceError(`No slot named '${i.slot_name}' in this deck`);
            slotId = slot.id;
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
        return {
          result: `Proposal #${id} created with ${items.length} item(s). The owner will rule on it — do not assume acceptance.`,
          isError: false,
          mutatedState: true,
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
