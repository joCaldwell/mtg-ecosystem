// Agent tools (spec §6.2/§6.3): mutations take oracle_id, never names —
// the only way to add a card is to have found it in a search result first.
// Searches are pre-filtered by deck color identity and exclude hard filters.
//
// The slot tools are the one exception to "the agent proposes, the owner
// rules", and only because slots are not deck content (spec §4): creating a
// slot or filing a card into one changes nothing about which cards are in the
// 100, bumps no revision, and is undone with a dropdown. Gating filing behind
// the same ceremony as a cut would make the gate mean less, not more.
// Membership — add, cut, maybe — stays behind propose_changes, always.
//
// Each entry in TOOLS holds a tool's schema and its handler side by side;
// TOOL_DEFS and executeTool are both derived from it, so a tool cannot exist
// with a def and no handler (or the reverse), and its name exists once, as
// the registry key.

import type { DatabaseSync } from "node:sqlite";
import { search } from "../search/index.ts";
import { createProposal, MAX_ITEMS } from "../deck/proposals.ts";
import { getCardHistory } from "../deck/log.ts";
import { proposeBriefEdit } from "../deck/brief.ts";
import { AppError } from "../errors.ts";
import {
  createSlot,
  deckCardIndex,
  deleteSlot,
  findSlotByName,
  getDeck,
  moveCards,
  requireDeck,
  updateSlot,
  type DeckCardRef,
  type SlotRow,
} from "../deck/service.ts";
import { normalizeName } from "../db.ts";
import {
  getAuditRun,
  listAuditRuns,
  lookupFinding,
  type AuditRun,
  type FoundFinding,
} from "../deck/audit.ts";
import { statline } from "./context.ts";
import type { ToolDef } from "./llm.ts";

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

// ---------- slots ----------

function slotNames(db: DatabaseSync, deckId: number): string[] {
  return getDeck(db, deckId).slots.map((s) => s.name);
}

// Rendered from the computed state, never counted here — the model is told
// the computed state is authoritative, so a tool that recounted and disagreed
// would be worse than one that reported nothing.
function describeSlots(db: DatabaseSync, deckId: number): string {
  const computed = getDeck(db, deckId).computed;
  const lines = computed.slot_deltas.map((s) => {
    if (s.status === "untargeted") return `- ${s.name}: ${s.count} (no target)`;
    const target = `${s.target_min ?? 0}–${s.target_max ?? "∞"}`;
    const status = s.status === "ok" ? "ok" : `${s.status} by ${Math.abs(s.delta)}`;
    return `- ${s.name}: ${s.count} (target ${target}, ${status})`;
  });
  lines.push(`- (unslotted): ${computed.unslotted_count}`);
  return lines.join("\n");
}

// Cards are addressed by name here, not oracle_id, because the decklist in
// context prints names and not ids — the agent has no id for a card it did not
// just search for. That is safe precisely because this tool only ever moves
// cards already in the deck (see deckCardIndex).
interface PlannedMove {
  card: DeckCardRef;
  slot: SlotRow | null;
}

// Resolve the whole request before touching anything: a bulk move that half
// applied would leave the deck in a state neither of us asked for, and the
// model cannot see the decklist update mid-turn to find out which half.
function planMoves(db: DatabaseSync, deckId: number, args: any): PlannedMove[] | string {
  const raw = Array.isArray(args?.moves) ? args.moves : args?.cards || args?.oracle_ids ? [args] : [];
  if (!raw.length) return "Error: move_cards needs a 'moves' array, each entry with a 'cards' list.";

  const { byId, byName } = deckCardIndex(db, deckId);
  const errors: string[] = [];
  const planned: PlannedMove[] = [];
  const seen = new Map<string, string>(); // oracle_id -> destination label

  for (const move of raw) {
    const rawName = move?.slot_name;
    let slot: SlotRow | null = null;
    if (rawName != null && String(rawName).trim() !== "") {
      slot = findSlotByName(db, deckId, String(rawName).trim());
      // 'none'/'unslotted' are how a model spells "no slot" when the schema
      // says the field is a string; neither can be a real slot name here,
      // since the lookup above already missed.
      if (!slot && !["none", "unslotted", "null"].includes(String(rawName).trim().toLowerCase())) {
        const known = slotNames(db, deckId);
        errors.push(
          `No slot named '${rawName}' in this deck. Existing slots: ${
            known.length ? known.join(", ") : "(none)"
          }. Create it with create_slot first.`,
        );
        continue;
      }
    }
    const label = slot ? slot.name : "unslotted";

    const list = Array.isArray(move?.cards)
      ? move.cards
      : Array.isArray(move?.oracle_ids)
        ? move.oracle_ids
        : move?.cards != null
          ? [move.cards]
          : [];
    if (!list.length) errors.push(`The move to '${label}' listed no cards.`);

    for (const entry of list) {
      const ref = String(entry ?? "").trim();
      const card = byId.get(ref) ?? byName.get(normalizeName(ref));
      if (!card) {
        errors.push(`'${ref}' is not a card in this deck — move_cards only refiles cards already in it.`);
        continue;
      }
      if (card.role !== "card") {
        errors.push(
          `[[${card.name}]] is in the command zone (${card.role}), which is not part of any slot.`,
        );
        continue;
      }
      const prior = seen.get(card.oracle_id);
      if (prior != null && prior !== label) {
        errors.push(`[[${card.name}]] was sent to both '${prior}' and '${label}' — pick one.`);
        continue;
      }
      if (prior != null) continue;
      seen.set(card.oracle_id, label);
      planned.push({ card, slot });
    }
  }

  if (errors.length)
    return `Error: nothing was moved — the whole call is rejected so the deck never ends up half-refiled.\n${errors
      .map((e) => `- ${e}`)
      .join("\n")}`;
  return planned;
}

// ---------- the registry ----------

interface AgentTool {
  description: string;
  parameters: Record<string, unknown>;
  handler: (db: DatabaseSync, deckId: number, args: any) => ToolOutcome;
}

const TOOLS: Record<string, AgentTool> = {
  search_cards: {
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
    handler(db, deckId, args) {
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
      const lines = results.map(
        (c) =>
          `[[${c.name}]] ${c.mana_cost ?? ""} — ${c.type_line}${statline(c)} (id: ${c.color_identity || "C"})\noracle_id: ${c.oracle_id}\n${c.oracle_text}`,
      );
      return {
        result: `${results.length} result(s):\n\n${lines.join("\n\n")}`,
        isError: false,
        mutatedState: false,
      };
    },
  },

  propose_changes: {
    description:
      `Propose deck changes for the owner to rule on. 3–${MAX_ITEMS} items maximum — rank your best ideas. oracle_id values MUST come from this conversation's search results or the decklist. Items sharing a group_id are accepted/rejected as a unit (use for swaps). Slots are optional — never withhold a proposal for want of one.`,
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
    handler(db, deckId, args) {
      // Slots are an optional organizational overlay (spec §4) — a proposal
      // never requires one. An unknown slot name downgrades that item to
      // unslotted instead of failing the whole proposal; the owner can slot
      // it after accepting. Failing here taught the agent that a deck with no
      // slots couldn't be proposed to at all.
      const unknownSlots: string[] = [];
      const items = (args.items ?? []).map((i: any) => {
        let slotId: number | null = null;
        if (i.slot_name) {
          const slot = findSlotByName(db, deckId, String(i.slot_name));
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
    },
  },

  propose_brief_edit: {
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
    handler(db, deckId, args) {
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
    },
  },

  propose_engine_edit: {
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
    handler(db, deckId, args) {
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
    },
  },

  get_audit: {
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
    handler(db, deckId, args) {
      const revision = requireDeck(db, deckId).revision;
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
    },
  },

  create_slot: {
    description:
      "Create a slot in this deck. APPLIES IMMEDIATELY — slots are organization, not deck content, so they are not proposals. Name what a card DOES ('ramp', 'interaction', 'card advantage'), never what it is: card types are reserved in both singular and plural, as are search prefixes. Creating a slot does not put anything in it — follow with move_cards.",
    parameters: {
      type: "object",
      properties: {
        name: { type: "string", description: "Slot name, e.g. 'interaction'" },
        target_min: { type: ["integer", "null"], description: "Optional lower bound on the slot's count" },
        target_max: { type: ["integer", "null"], description: "Optional upper bound on the slot's count" },
      },
      required: ["name"],
    },
    handler(db, deckId, args) {
      const slotName = String(args.name ?? "");
      if (findSlotByName(db, deckId, slotName))
        return {
          result: `A slot named '${slotName}' already exists — use update_slot to retarget it, or move_cards to file cards into it.`,
          isError: true,
          mutatedState: false,
        };
      createSlot(
        db,
        deckId,
        slotName,
        args.target_min == null ? null : Number(args.target_min),
        args.target_max == null ? null : Number(args.target_max),
      );
      const created = findSlotByName(db, deckId, slotName)!;
      return {
        result: `Slot '${created.name}' created (empty). Slots now:\n${describeSlots(db, deckId)}\n\nFile cards into it with move_cards, and set slot_name on future proposals that belong here.`,
        isError: false,
        mutatedState: true,
      };
    },
  },

  update_slot: {
    description:
      "Rename a slot or change its target counts. Applies immediately. A target is a claim about how this deck should be built, so say why you set it. Omit a target field to leave it as it is; pass null to clear it.",
    parameters: {
      type: "object",
      properties: {
        slot_name: { type: "string", description: "The slot to change, by its current name" },
        new_name: { type: "string", description: "Optional new name" },
        target_min: { type: ["integer", "null"], description: "New lower bound; null clears it" },
        target_max: { type: ["integer", "null"], description: "New upper bound; null clears it" },
      },
      required: ["slot_name"],
    },
    handler(db, deckId, args) {
      const slot = findSlotByName(db, deckId, String(args.slot_name ?? ""));
      if (!slot) {
        const known = slotNames(db, deckId);
        return {
          result: `No slot named '${args.slot_name}' in this deck. Existing slots: ${known.length ? known.join(", ") : "(none)"}.`,
          isError: true,
          mutatedState: false,
        };
      }
      // Absent means "leave it"; an explicit null means "clear it". Only
      // JSON.parse preserves that difference, so test presence, not value.
      const patch: Parameters<typeof updateSlot>[3] = {};
      if (args.new_name !== undefined && args.new_name !== null) patch.name = String(args.new_name);
      if ("target_min" in args) patch.targetMin = args.target_min == null ? null : Number(args.target_min);
      if ("target_max" in args) patch.targetMax = args.target_max == null ? null : Number(args.target_max);
      if (!Object.keys(patch).length)
        return {
          result: `Nothing to change on '${slot.name}' — pass new_name, target_min, or target_max.`,
          isError: true,
          mutatedState: false,
        };
      updateSlot(db, deckId, slot.id, patch);
      return {
        result: `Slot updated. Slots now:\n${describeSlots(db, deckId)}`,
        isError: false,
        mutatedState: true,
      };
    },
  },

  delete_slot: {
    description:
      "Delete a slot. Applies immediately. Cards filed in it are NOT removed from the deck — they become unslotted. This throws away the owner's filing for those cards, so do it when asked or when you can say plainly why the slot was wrong, not as housekeeping.",
    parameters: {
      type: "object",
      properties: { slot_name: { type: "string" } },
      required: ["slot_name"],
    },
    handler(db, deckId, args) {
      const slot = findSlotByName(db, deckId, String(args.slot_name ?? ""));
      if (!slot) {
        const known = slotNames(db, deckId);
        return {
          result: `No slot named '${args.slot_name}' in this deck. Existing slots: ${known.length ? known.join(", ") : "(none)"}.`,
          isError: true,
          mutatedState: false,
        };
      }
      // Name what falls out of the slot before it does. deck_cards.slot_id is
      // ON DELETE SET NULL, so the filing is gone the moment the row is; the
      // list in the transcript is what makes it restorable.
      const orphaned = (
        db
          .prepare(
            `SELECT c.name FROM deck_cards dc JOIN cards c ON c.oracle_id = dc.oracle_id
             WHERE dc.deck_id = ? AND dc.slot_id = ? ORDER BY c.name`,
          )
          .all(deckId, slot.id) as unknown as Array<{ name: string }>
      ).map((r) => r.name);
      deleteSlot(db, deckId, slot.id);
      const fallout = orphaned.length
        ? ` ${orphaned.length} card(s) are now unslotted and still in the deck: ${orphaned.map((n) => `[[${n}]]`).join(", ")}.`
        : " It was empty.";
      return {
        result: `Slot '${slot.name}' deleted.${fallout}\nSlots now:\n${describeSlots(db, deckId)}`,
        isError: false,
        mutatedState: true,
      };
    },
  },

  move_cards: {
    description:
      "File cards that are ALREADY IN THE DECK into slots. Applies immediately, as one transaction — either every move lands or none does. This is bulk: put every card that belongs in a slot into ONE call rather than calling once per card, and use several entries in `moves` to refile the whole deck at once. Cards are named exactly as the decklist spells them (oracle_ids also work). To take a card out of its slot, omit slot_name for that entry. This does not add or cut anything — use propose_changes for that.",
    parameters: {
      type: "object",
      properties: {
        moves: {
          type: "array",
          description: "One entry per destination slot",
          items: {
            type: "object",
            properties: {
              slot_name: {
                type: ["string", "null"],
                description: "Destination slot, which must already exist. Omit or null to unslot these cards.",
              },
              cards: {
                type: "array",
                items: { type: "string" },
                description: "Card names as the decklist spells them, or oracle_ids",
              },
            },
            required: ["cards"],
          },
        },
      },
      required: ["moves"],
    },
    handler(db, deckId, args) {
      const planned = planMoves(db, deckId, args);
      if (typeof planned === "string")
        return { result: planned, isError: true, mutatedState: false };
      if (!planned.length)
        return { result: "No cards to move.", isError: true, mutatedState: false };

      moveCards(
        db,
        deckId,
        planned.map((m) => ({ oracle_id: m.card.oracle_id, slot_id: m.slot?.id ?? null })),
      );

      const byDest = new Map<string, string[]>();
      for (const m of planned) {
        const label = m.slot ? m.slot.name : "unslotted";
        byDest.set(label, [...(byDest.get(label) ?? []), m.card.name]);
      }
      const moved = [...byDest]
        .map(([label, names]) => `- → ${label}: ${names.map((n) => `[[${n}]]`).join(", ")}`)
        .join("\n");
      return {
        result: `Moved ${planned.length} card(s):\n${moved}\n\nSlots now:\n${describeSlots(db, deckId)}\n\nDeck membership is unchanged — this only refiled cards already in the deck.`,
        isError: false,
        mutatedState: true,
      };
    },
  },

  get_card_history: {
    description:
      "Get the full decision history for one card in this deck (accepts, rejections with types and reasons, undos). Required before re-proposing anything previously rejected.",
    parameters: {
      type: "object",
      properties: {
        oracle_id: { type: "string" },
      },
      required: ["oracle_id"],
    },
    handler(db, deckId, args) {
      const history = getCardHistory(db, deckId, String(args.oracle_id ?? "")) as any[];
      if (!history.length)
        return { result: "No decisions recorded for this card.", isError: false, mutatedState: false };
      const lines = history.map((e) => {
        if (e.kind === "reject")
          return `${e.ts}: rejected ${e.action} (${e.rejection_type}): "${e.rejection_reason}"`;
        return `${e.ts}: ${e.kind} ${e.action ?? ""}${e.rationale ? ` — ${e.rationale}` : ""}`;
      });
      return { result: lines.join("\n"), isError: false, mutatedState: false };
    },
  },
};

export const TOOL_DEFS: ToolDef[] = Object.entries(TOOLS).map(([name, t]) => ({
  type: "function",
  function: { name, description: t.description, parameters: t.parameters },
}));

export function executeTool(
  db: DatabaseSync,
  deckId: number,
  name: string,
  args: any,
): ToolOutcome {
  const tool = TOOLS[name];
  if (!tool) return { result: `Unknown tool: ${name}`, isError: true, mutatedState: false };
  try {
    return tool.handler(db, deckId, args ?? {});
  } catch (e: any) {
    // Typed errors are the model's to fix (bad query syntax, a stale id);
    // anything else is a real bug and propagates.
    if (e instanceof AppError) {
      return { result: `Error: ${e.message}`, isError: true, mutatedState: false };
    }
    throw e;
  }
}
