import { useEffect, useRef, useState, type FormEvent } from "react";
import { api, type ChatMsg, type DeckState, type Slot } from "./api.ts";
import { Markdown } from "./Markdown.tsx";
import { ProposalCard } from "./ProposalCard.tsx";

// Audit references the owner dropped in from the audit section. They stay in
// the transcript as the token the agent resolved (audit#12/reasoning:…), so
// the message means the same thing on replay — highlight, never rewrite.
const AUDIT_REF_BODY = "audit(?:#\\d+)?/[A-Za-z0-9_:.-]+";
const AUDIT_REF_WHOLE = new RegExp(`^${AUDIT_REF_BODY}$`);

function AuditRefText({ text }: { text: string }) {
  return (
    <>
      {text.split(new RegExp(`(${AUDIT_REF_BODY})`)).map((p, i) =>
        AUDIT_REF_WHOLE.test(p) ? (
          <span key={i} className="auditref" title="Audit finding reference">
            {p}
          </span>
        ) : (
          <span key={i}>{p}</span>
        ),
      )}
    </>
  );
}

// `carded` holds the tool_call_ids whose proposal is rendered in full further
// down the transcript. Summarising those as "proposed 3 change(s)" directly
// above the proposal itself is noise — but turns from before proposals were
// linked to their message have no card, so they keep the line.
function toolSummary(m: ChatMsg, carded: Set<string>): string | null {
  if (m.role !== "assistant" || !m.tool_calls?.length) return null;
  const parts = m.tool_calls
    .filter((c) => !carded.has(c.id))
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
    });
  return parts.length ? parts.join(" · ") : null;
}

export function ChatPanel({
  deckId,
  setState,
  input,
  setInput,
  slots,
  mutate,
}: {
  deckId: number;
  setState: (s: DeckState) => void;
  // The draft lives on the page, not here: the audit section writes audit#…
  // references into it, and the panel unmounts every time you flip to Search.
  input: string;
  setInput: (v: string) => void;
  // Both only for the proposals rendered inline: slot names, and ruling on an
  // item without leaving the conversation it came from.
  slots: Slot[];
  mutate: (fn: () => Promise<DeckState>) => Promise<void>;
}) {
  const [history, setHistory] = useState<ChatMsg[]>([]);
  const [busy, setBusy] = useState(false);
  /** The message just sent, shown until the reloaded transcript carries it. */
  const [sent, setSent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    api.getChat(deckId).then(setHistory).catch(() => {});
  }, [deckId]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [history, busy, sent]);

  async function send(e: FormEvent) {
    e.preventDefault();
    const message = input.trim();
    if (!message || busy) return;
    setInput("");
    // The turn takes a model call to come back, and the transcript only
    // reloads when it does — so the message goes up optimistically. The
    // server persists it before the first model call either way, so the
    // reload that clears this shows the same text back.
    setSent(message);
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
      // Batched with the setHistory above, so the optimistic copy is replaced
      // by the persisted one in one paint rather than blinking out first.
      setBusy(false);
      setSent(null);
    }
  }

  // A proposal is made mid-turn (a tool message) but reads as the conclusion of
  // the reply that argues for it, so it is carried forward to the foot of that
  // reply rather than rendered where it falls. A turn that produced a proposal
  // but never reached a final reply — an error, a lint bounce that gave up —
  // leaves it orphaned, and those render on their own so nothing is lost.
  const attached = new Map<number, NonNullable<ChatMsg["proposal"]>[]>();
  const orphans = new Set<number>();
  {
    let carrying: ChatMsg[] = [];
    const strand = () => {
      carrying.forEach((t) => orphans.add(t.id));
      carrying = [];
    };
    for (const m of history) {
      if (m.role === "user") strand();
      else if (m.role === "tool" && m.proposal) carrying.push(m);
      // The turn's final reply is the assistant message with prose and no
      // further tool calls.
      else if (m.role === "assistant" && m.content && !m.tool_calls?.length && carrying.length) {
        attached.set(m.id, carrying.map((t) => t.proposal!));
        carrying = [];
      }
    }
    strand();
  }

  // Tool messages are otherwise plumbing and stay hidden; an orphaned proposal
  // is the exception, because the proposal is the point of the turn.
  const visible = history.filter(
    (m) =>
      m.role === "user" ||
      (m.role === "assistant" && (m.content || m.tool_calls?.length)) ||
      (m.role === "tool" && m.proposal && orphans.has(m.id)),
  );
  // Compacted messages left the model's context but never left the disk
  // (spec §11) — show them collapsed so that's visible, not implied.
  const compacted = visible.filter((m) => m.compacted_at);
  const resident = visible.filter((m) => !m.compacted_at);

  const carded = new Set(
    history.filter((m) => m.proposal && m.tool_call_id).map((m) => m.tool_call_id!),
  );

  // A ruling made here has to move both surfaces: the deck (via mutate) and
  // this transcript, whose cards show each item's status.
  async function ruleFromChat(fn: () => Promise<DeckState>) {
    await mutate(fn);
    setHistory(await api.getChat(deckId).catch(() => history));
  }

  function Message({ m }: { m: ChatMsg }) {
    if (m.role === "user") {
      const text = (m.content ?? "").replace(/^<state_summary>.*?<\/state_summary>\n\n/s, "");
      return (
        <div className="chat-msg user">
          <AuditRefText text={text} />
        </div>
      );
    }
    // An orphan still gets a bubble of its own — a proposal is never a loose
    // panel floating at the width of the column.
    if (m.role === "tool")
      return m.proposal ? (
        <div className="chat-msg assistant">
          <ProposalBlock proposal={m.proposal} />
        </div>
      ) : null;

    const tools = toolSummary(m, carded);
    if (tools) return <div className="chat-tool muted">⚙ {tools}</div>;
    const proposals = attached.get(m.id);
    if (m.content || proposals?.length)
      return (
        <div className="chat-msg assistant">
          {m.content && <Markdown text={m.content} />}
          {proposals?.map((p) => <ProposalBlock key={p.id} proposal={p} />)}
        </div>
      );
    return null;
  }

  function ProposalBlock({ proposal }: { proposal: NonNullable<ChatMsg["proposal"]> }) {
    const open = proposal.items.filter((i) => i.status === "pending").length;
    return (
      <div className="msg-proposal">
        <div className="msg-proposal-head">
          <span>
            Proposal #{proposal.id} · {proposal.items.length} item(s)
          </span>
          <span className={`chip ${open ? "under" : "ok"}`}>
            {open ? `${open} awaiting you` : "ruled"}
          </span>
        </div>
        {proposal.note && <div className="muted rationale">{proposal.note}</div>}
        <ProposalCard
          proposal={proposal}
          deckId={deckId}
          slots={slots}
          rule={ruleFromChat}
          head={false}
        />
      </div>
    );
  }

  return (
    <div className="chat">
      <div className="chat-messages">
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
        {sent !== null && (
          <div className="chat-msg user">
            <AuditRefText text={sent} />
          </div>
        )}
        {busy && <div className="chat-tool muted">thinking…</div>}
        <div ref={bottomRef} />
      </div>
      {error && <div className="error-banner">{error}</div>}
      {/* The composer is one field: the form draws the border and focus ring,
          the input inside it is chromeless. */}
      <form onSubmit={send} className="chat-input">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Ask the agent… it proposes, you rule"
          disabled={busy}
        />
        <button className="primary" type="submit" disabled={busy || !input.trim()}>
          Send
        </button>
      </form>
    </div>
  );
}
