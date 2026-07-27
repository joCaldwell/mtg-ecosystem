import { useEffect, useRef, useState, type FormEvent } from "react";
import { api, type ChatMsg, type DeckState } from "./api.ts";

// Render [[Card Name]] as highlighted chips — lint guarantees these resolve.
export function CardText({ text }: { text: string }) {
  const parts = text.split(/(\[\[[^\[\]]+\]\])/g);
  return (
    <>
      {parts.map((p, i) => {
        const m = p.match(/^\[\[([^\[\]]+)\]\]$/);
        return m ? (
          <span key={i} className="cardref">
            {m[1]}
          </span>
        ) : (
          <span key={i}>{p}</span>
        );
      })}
    </>
  );
}

function toolSummary(m: ChatMsg): string | null {
  if (m.role === "assistant" && m.tool_calls?.length) {
    return m.tool_calls
      .map((c) => {
        try {
          const args = JSON.parse(c.function.arguments || "{}");
          if (c.function.name === "search_cards") return `search: ${args.query}`;
          if (c.function.name === "propose_changes")
            return `proposed ${args.items?.length ?? 0} change(s)`;
          return c.function.name;
        } catch {
          return c.function.name;
        }
      })
      .join(" · ");
  }
  return null;
}

export function ChatPanel({
  deckId,
  setState,
}: {
  deckId: number;
  setState: (s: DeckState) => void;
}) {
  const [history, setHistory] = useState<ChatMsg[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api.getChat(deckId).then(setHistory).catch(() => {});
  }, [deckId]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [history, busy]);

  async function send(e: FormEvent) {
    e.preventDefault();
    const message = input.trim();
    if (!message || busy) return;
    setInput("");
    setBusy(true);
    setError(null);
    try {
      const r = await api.sendChat(deckId, message);
      setState(r.state);
      setHistory(await api.getChat(deckId));
    } catch (e: any) {
      setError(e.message);
      setHistory(await api.getChat(deckId).catch(() => history));
    } finally {
      setBusy(false);
    }
  }

  const visible = history.filter(
    (m) => m.role === "user" || (m.role === "assistant" && (m.content || m.tool_calls?.length)),
  );
  // Compacted messages left the model's context but never left the disk
  // (spec §11) — show them collapsed so that's visible, not implied.
  const compacted = visible.filter((m) => m.compacted_at);
  const resident = visible.filter((m) => !m.compacted_at);

  function Message({ m }: { m: ChatMsg }) {
    if (m.role === "user") {
      const text = (m.content ?? "").replace(/^<state_summary>.*?<\/state_summary>\n\n/s, "");
      return <div className="chat-msg user">{text}</div>;
    }
    const tools = toolSummary(m);
    if (tools) return <div className="chat-tool muted">⚙ {tools}</div>;
    if (m.content)
      return (
        <div className="chat-msg assistant">
          <CardText text={m.content} />
        </div>
      );
    return null;
  }

  return (
    <div className="chat">
      <div className="chat-messages">
        {!history.length && (
          <p className="muted">
            One chat per deck, forever. The agent sees the full decklist, brief, and your
            decision history — and can only reference cards it has actually looked up.
          </p>
        )}
        {compacted.length > 0 && (
          <details className="compacted-block">
            <summary className="muted">
              {compacted.length} message(s) compacted out of context — still on disk
            </summary>
            {compacted.map((m) => (
              <Message key={m.id} m={m} />
            ))}
          </details>
        )}
        {resident.map((m) => (
          <Message key={m.id} m={m} />
        ))}
        {busy && <div className="chat-tool muted">thinking…</div>}
        <div ref={bottomRef} />
      </div>
      {error && <div className="error-banner">{error}</div>}
      <form onSubmit={send} className="row gap chat-input">
        <input
          className="grow"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Ask the agent… (it proposes; you rule)"
          disabled={busy}
        />
        <button type="submit" disabled={busy || !input.trim()}>
          Send
        </button>
      </form>
    </div>
  );
}
