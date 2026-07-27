// Minimal OpenRouter client (OpenAI-compatible chat completions). Plain
// fetch, no SDK — the transport is injectable so agent tests never touch
// the network.

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

export class LlmError extends Error {}

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
