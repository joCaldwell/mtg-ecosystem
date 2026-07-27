import type { DatabaseSync } from "node:sqlite";
import { assembleContext } from "./context.ts";
import { TOOL_DEFS, executeTool } from "./tools.ts";
import { lintOutput, lintCorrectionMessage } from "./lint.ts";
import { getProposal } from "../deck/proposals.ts";
import { LlmError, type ChatMessage, type ChatTransport } from "./llm.ts";

const MAX_MODEL_CALLS = 12;
const MAX_LINT_BOUNCES = 2;

export class AgentError extends Error {}

export interface TurnResult {
  reply: string;
  mutatedState: boolean;
  modelCalls: number;
}

// One agent turn: assemble context, loop tool calls, lint the final text.
// Unresolved [[Card Name]]s are bounced back to the model and NEVER reach
// the caller (spec §6.4).
export async function runTurn(
  db: DatabaseSync,
  deckId: number,
  userText: string,
  transport: ChatTransport,
  retentionN: number,
): Promise<TurnResult> {
  const { system, transcript, tailRestate } = assembleContext(db, deckId, retentionN);

  // `extra` is stored alongside the message but deliberately not pushed into
  // `messages` — the provider gets exactly the fields it knows about, while the
  // row on disk keeps the link the UI needs.
  const persist = (msg: ChatMessage, extra?: Record<string, unknown>) => {
    db.prepare("INSERT INTO chat_messages (deck_id, role, content_json) VALUES (?, ?, ?)").run(
      deckId,
      msg.role,
      JSON.stringify(extra ? { ...msg, ...extra } : msg),
    );
  };

  // Tail restate rides immediately before the owner's message (spec §10).
  const userMessage: ChatMessage = {
    role: "user",
    content: `<state_summary>${tailRestate}</state_summary>\n\n${userText}`,
  };
  persist(userMessage);

  const messages: ChatMessage[] = [
    { role: "system", content: system },
    ...transcript,
    userMessage,
  ];

  let mutatedState = false;
  let modelCalls = 0;
  let lintBounces = 0;

  while (true) {
    if (modelCalls >= MAX_MODEL_CALLS)
      throw new AgentError(
        `Turn exceeded ${MAX_MODEL_CALLS} model calls without completing — aborted to bound cost.`,
      );
    modelCalls++;

    const response = await transport({ messages, tools: TOOL_DEFS });
    const assistant = response.message;

    if (response.finish_reason === "length")
      throw new AgentError("The model ran out of output tokens mid-reply. Try again.");
    if (response.finish_reason === "content_filter")
      throw new AgentError("The model's reply was blocked by the provider's content filter.");

    persist(assistant);
    messages.push(assistant);

    if (assistant.tool_calls?.length) {
      for (const call of assistant.tool_calls) {
        let args: any = {};
        let outcome;
        try {
          args = JSON.parse(call.function.arguments || "{}");
        } catch {
          outcome = { result: "Error: tool arguments were not valid JSON.", isError: true, mutatedState: false };
        }
        outcome ??= executeTool(db, deckId, call.function.name, args);
        if (outcome.mutatedState) mutatedState = true;
        const toolMsg: ChatMessage = {
          role: "tool",
          tool_call_id: call.id,
          content: outcome.result,
        };
        persist(toolMsg, outcome.proposalId != null ? { proposal_id: outcome.proposalId } : undefined);
        messages.push(toolMsg);
      }
      continue;
    }

    // Final text — lint before it can reach a screen.
    const text = assistant.content ?? "";
    const lint = lintOutput(db, text);
    if (lint.ok) return { reply: text, mutatedState, modelCalls };

    if (lintBounces >= MAX_LINT_BOUNCES)
      throw new AgentError(
        `The agent repeatedly referenced nonexistent cards (${lint.failures.map((f) => f.name).join(", ")}). Nothing was shown; try rephrasing your request.`,
      );
    lintBounces++;
    const correction: ChatMessage = { role: "system", content: lintCorrectionMessage(lint.failures) };
    persist(correction);
    messages.push(correction);
  }
}

export function getChatHistory(db: DatabaseSync, deckId: number) {
  const rows = db
    .prepare(
      "SELECT id, role, content_json, compacted_at, created_at FROM chat_messages WHERE deck_id = ? ORDER BY id",
    )
    .all(deckId) as unknown as Array<{
    id: number;
    role: string;
    content_json: string;
    compacted_at: string | null;
    created_at: string;
  }>;
  // Compacted messages are still returned — they stay on disk and the UI
  // shows them collapsed, so compaction is visibly non-destructive (§11).
  const messages = rows.map((r) => ({
    id: r.id,
    created_at: r.created_at,
    compacted_at: r.compacted_at,
    ...(JSON.parse(r.content_json) as ChatMessage & { proposal_id?: number }),
  }));
  // Hydrate the proposals a turn produced, read live rather than frozen at
  // send time: the transcript then shows each item's current ruling, and the
  // owner can rule from the chat without going to the decision log to find out
  // whether anything was proposed at all.
  const cache = new Map<number, ReturnType<typeof getProposal>>();
  return messages.map((m) => {
    if (m.proposal_id == null) return m;
    if (!cache.has(m.proposal_id)) cache.set(m.proposal_id, getProposal(db, m.proposal_id));
    return { ...m, proposal: cache.get(m.proposal_id) ?? null };
  });
}

export { LlmError };
