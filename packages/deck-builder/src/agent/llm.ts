// Minimal OpenRouter client (OpenAI-compatible chat completions). Plain
// fetch, no SDK — the transport is injectable so agent tests never touch
// the network.

import { AppError } from "../errors.ts";

export interface ToolCall {
  id: string;
  type: "function";
  function: { name: string; arguments: string };
}

export interface ChatMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string | null;
  tool_calls?: ToolCall[];
  tool_call_id?: string;
}

export interface ToolDef {
  type: "function";
  function: {
    name: string;
    description: string;
    parameters: Record<string, unknown>;
  };
}

export interface ChatResponse {
  message: ChatMessage;
  finish_reason: string;
  usage?: { prompt_tokens: number; completion_tokens: number };
  model?: string;
}

export type ChatTransport = (req: {
  messages: ChatMessage[];
  tools: ToolDef[];
  json?: boolean;
}) => Promise<ChatResponse>;

export class LlmError extends AppError {
  constructor(message: string) {
    super(message, 502);
  }
}

export function openRouterTransport(cfg: {
  apiKey: string;
  model: string;
  baseUrl: string;
}): ChatTransport {
  return async ({ messages, tools, json }) => {
    const res = await fetch(`${cfg.baseUrl}/chat/completions`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${cfg.apiKey}`,
        "Content-Type": "application/json",
        "X-Title": "MTG Deck Builder",
      },
      body: JSON.stringify({
        model: cfg.model,
        messages,
        tools: tools.length ? tools : undefined,
        response_format: json ? { type: "json_object" } : undefined,
      }),
    });

    const body = (await res.json().catch(() => null)) as any;
    if (!res.ok) {
      const detail = body?.error?.message ?? `HTTP ${res.status}`;
      throw new LlmError(`OpenRouter request failed: ${detail}`);
    }
    // OpenRouter can also return 200 with an error payload
    if (body?.error) throw new LlmError(`OpenRouter error: ${body.error.message ?? "unknown"}`);
    const choice = body?.choices?.[0];
    if (!choice?.message) throw new LlmError("OpenRouter returned no message");
    return {
      message: choice.message as ChatMessage,
      finish_reason: choice.finish_reason ?? "stop",
      usage: body.usage,
      model: body.model,
    };
  };
}

// ---------- structured (JSON-mode) calls ----------

// Strip the markdown fences models sometimes add despite instructions.
export function parseModelJson(text: string): any {
  const cleaned = text
    .trim()
    .replace(/^```(?:json)?\s*/i, "")
    .replace(/```\s*$/, "");
  return JSON.parse(cleaned);
}

// One JSON-mode round-trip with a single correction retry, shared by every
// pass that wants a structured reply (consolidation, audit reasoning). An
// unparseable response — or a validate() complaint — is bounced back to the
// model once with `correction`; a second failure comes back as `error` and
// the caller decides whether that throws or degrades.
export async function callJson<T = any>(
  transport: ChatTransport,
  messages: ChatMessage[],
  validate?: (parsed: any) => { correction: string; failure: string } | null,
): Promise<{ parsed: T; error?: never } | { parsed?: never; error: string }> {
  const msgs = [...messages];
  for (let attempt = 0; attempt < 2; attempt++) {
    const response = await transport({ messages: msgs, tools: [], json: true });
    const text = response.message.content ?? "";
    let candidate: any;
    try {
      candidate = parseModelJson(text);
    } catch {
      if (attempt === 1) break;
      msgs.push(response.message, {
        role: "system",
        content: "That was not valid JSON. Respond again with ONLY the JSON object.",
      });
      continue;
    }
    const complaint = validate?.(candidate) ?? null;
    if (complaint) {
      if (attempt === 1) return { error: complaint.failure };
      msgs.push(response.message, { role: "system", content: complaint.correction });
      continue;
    }
    return { parsed: candidate as T };
  }
  return { error: "The model did not return valid JSON after a retry." };
}
