import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { api, type ChatMsg, type Envelope } from "./api.ts";
import { useDeck } from "./store.tsx";
import { useAutosize } from "./lib.ts";
import { Markdown } from "./Markdown.tsx";
import { BriefEditCard } from "./BriefEditCard.tsx";
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

// `actionable` holds tool_call_ids whose proposal/edit is rendered in full
// further down the transcript. A summary directly above its ruling card is
// noise, while older turns from before links existed still keep their line.
function toolSummary(m: ChatMsg, actionable: Set<string>): string | null {
  if (m.role !== "assistant" || !m.tool_calls?.length) return null;
  const parts = m.tool_calls
    .filter((c) => !actionable.has(c.id))
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

// Message and the ruling blocks live at module scope, not as closures inside
// ChatPanel: an inline component gets a new identity every render, and React
// would remount the subtree — resetting an open reject form whenever a
// keystroke landed in the composer.

function ProposalBlock({
  proposal,
  rule,
}: {
  proposal: NonNullable<ChatMsg["proposal"]>;
  rule: (fn: () => Promise<Envelope>) => Promise<unknown>;
}) {
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
      {proposal.note && <div className="muted rationale"><Markdown text={proposal.note} /></div>}
      <ProposalCard proposal={proposal} rule={rule} head={false} />
    </div>
  );
}

function BriefEditBlock({
  edit,
  rule,
}: {
  edit: NonNullable<ChatMsg["brief_edit"]>;
  rule: (fn: () => Promise<Envelope>) => Promise<unknown>;
}) {
  const open = edit.status === "pending";
  return (
    <div className="msg-proposal">
      <div className="msg-proposal-head">
        <span>Brief edit #{edit.id}</span>
        <span className={`chip ${open ? "under" : edit.status === "accepted" ? "ok" : "over"}`}>
          {open ? "awaiting you" : edit.status}
        </span>
      </div>
      <BriefEditCard edit={edit} rule={rule} head={false} />
    </div>
  );
}

function ActionBlock({
  message,
  rule,
}: {
  message: ChatMsg;
  rule: (fn: () => Promise<Envelope>) => Promise<unknown>;
}) {
  if (message.proposal) return <ProposalBlock proposal={message.proposal} rule={rule} />;
  if (message.brief_edit) return <BriefEditBlock edit={message.brief_edit} rule={rule} />;
  return null;
}

function Message({
  m,
  attached,
  rule,
}: {
  m: ChatMsg;
  /** Ruling cards carried to the foot of their final reply, keyed by message id. */
  attached: Map<number, ChatMsg[]>;
  rule: (fn: () => Promise<Envelope>) => Promise<unknown>;
}) {
  if (m.role === "user") {
    const text = (m.content ?? "").replace(/^<state_summary>.*?<\/state_summary>\n\n/s, "");
    return (
      <div className="chat-msg user">
        <AuditRefText text={text} />
      </div>
    );
  }
  // An orphan still gets a bubble of its own — a ruling card is never a loose
  // panel floating at the width of the column.
  if (m.role === "tool")
    return m.proposal || m.brief_edit ? (
      <div className="chat-msg assistant">
        <ActionBlock message={m} rule={rule} />
      </div>
    ) : null;

  const actions = attached.get(m.id);
  if (m.content || actions?.length)
    return (
      <div className="chat-msg assistant">
        {m.content && <Markdown text={m.content} />}
        {actions?.map((action) => (
          <ActionBlock key={action.id} message={action} rule={rule} />
        ))}
      </div>
    );
  return null;
}

export function ChatPanel({
  insertRef,
}: {
  /** Set by the page so the audit section can drop an audit#… reference into
   *  the composer draft without owning it. A handle rather than a lifted
   *  value: the draft is this panel's business. */
  insertRef?: { current: ((token: string) => void) | null };
}) {
  const { deckId, apply, run } = useDeck();
  const [history, setHistory] = useState<ChatMsg[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  /** The message just sent, shown until the reloaded transcript carries it. */
  const [sent, setSent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const boxRef = useRef<HTMLTextAreaElement>(null);
  /** Was the transcript parked at its foot when it was last scrolled? */
  const pinned = useRef(true);

  useEffect(() => {
    api.getChat(deckId).then(setHistory).catch(() => {});
  }, [deckId]);

  useEffect(() => {
    if (!insertRef) return;
    insertRef.current = (token) =>
      setInput((d) => (d.trim() ? `${d.trimEnd()} ${token} ` : `${token} `));
    return () => {
      insertRef.current = null;
    };
  }, [insertRef]);

  // Following the transcript down is only ever wanted when you are already
  // reading its foot: `history` is replaced on every ruling too, and accepting
  // a proposal partway up the conversation must leave you next to the thing
  // you just ruled on rather than throwing you to the bottom.
  //
  // Written straight onto the container before paint, rather than animated
  // into place by scrollIntoView on a trailing anchor. The animation was the
  // jarring part; it also raced the panel's own layout on mount — losing, and
  // leaving the transcript sitting at the very top — and scrollIntoView walks
  // every scrollable ancestor, so it could move the page as well as the pane
  // it was aimed at.
  useLayoutEffect(() => {
    const el = listRef.current;
    if (!el || !pinned.current) return;
    el.scrollTop = el.scrollHeight;
  }, [history, busy, sent]);

  // Scrolling is the only thing that changes the answer. The pin above fires
  // this too, landing at the bottom — which is the value it should hold.
  function onTranscriptScroll() {
    const el = listRef.current;
    if (el) pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < 64;
  }

  // The panel stays mounted behind the Search tab (`display: none`), so the
  // transcript usually loads while it has no layout at all — and a scroll
  // position cannot be written to a box with no height, which left the chat
  // opening at its oldest message. So pin again whenever the box gets a size:
  // raising the tab, dragging the panel wider, maximizing it over the page.
  useEffect(() => {
    const el = listRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      if (pinned.current) el.scrollTop = el.scrollHeight;
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // The composer is as tall as what's in it. CSS keeps the floor (it opens
  // several lines tall) and the ceiling (past it, the box scrolls instead of
  // eating the transcript).
  useAutosize(boxRef, input);

  // A textarea takes Enter for itself, so the send key has to be put back by
  // hand: Enter sends, shift-Enter breaks the line. `isComposing` guards IME
  // input, where Enter is how you accept a candidate, not how you finish.
  function onComposerKey(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key !== "Enter" || e.shiftKey || e.nativeEvent.isComposing) return;
    e.preventDefault();
    // Through the form rather than send() directly, so submitting by key and
    // by button are the same path.
    e.currentTarget.form?.requestSubmit();
  }

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
      apply(r);
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

  // Approval-gated artifacts are made mid-turn (in tool messages) but read as
  // the conclusion of the reply that argues for them, so carry them to that
  // reply's foot. If the turn never reaches a final reply, render the orphan on
  // its own so the owner can still rule on it.
  const attached = new Map<number, ChatMsg[]>();
  const orphans = new Set<number>();
  {
    let carrying: ChatMsg[] = [];
    const strand = () => {
      carrying.forEach((t) => orphans.add(t.id));
      carrying = [];
    };
    for (const m of history) {
      if (m.role === "user") strand();
      else if (m.role === "tool" && (m.proposal || m.brief_edit)) carrying.push(m);
      // The turn's final reply is the assistant message with prose and no
      // further tool calls.
      else if (m.role === "assistant" && m.content && !m.tool_calls?.length && carrying.length) {
        attached.set(m.id, carrying);
        carrying = [];
      }
    }
    strand();
  }

  // Tool messages are otherwise plumbing and stay hidden; an orphaned ruling
  // card is the exception, because the proposed change is the point of the turn.
  const visible = history.filter(
    (m) =>
      m.role === "user" ||
      (m.role === "assistant" && (m.content || m.tool_calls?.length)) ||
      (m.role === "tool" && (m.proposal || m.brief_edit) && orphans.has(m.id)),
  );
  // Compacted messages left the model's context but never left the disk
  // (spec §11) — show them collapsed so that's visible, not implied.
  const compacted = visible.filter((m) => m.compacted_at);
  const resident = visible.filter((m) => !m.compacted_at);

  const actionable = new Set(
    history
      .filter((m) => (m.proposal || m.brief_edit) && m.tool_call_id)
      .map((m) => m.tool_call_id!),
  );

  // A ruling made here has to move both surfaces: the deck (via the store)
  // and this transcript, whose cards show each item's status.
  async function ruleFromChat(fn: () => Promise<Envelope>) {
    await run(fn);
    setHistory(await api.getChat(deckId).catch(() => history));
  }

  const msgProps = { attached, rule: ruleFromChat };

  // Keep consecutive tool activity behind one native disclosure. Stable keys
  // preserve its open state while typing or ruling on a proposal.
  function transcript(messages: ChatMsg[]): ReactNode[] {
    const nodes: ReactNode[] = [];
    let pending: { id: number; text: string; count: number }[] = [];
    function flush() {
      if (!pending.length) return;
      const count = pending.reduce((n, entry) => n + entry.count, 0);
      nodes.push(
        <details className="chat-tools" key={`tools-${pending[0].id}`}>
          <summary>{count} tool call{count === 1 ? "" : "s"}</summary>
          <div className="chat-tools-list">
            {pending.map((entry) => (
              <div className="chat-tool muted" key={entry.id}>{entry.text}</div>
            ))}
          </div>
        </details>,
      );
      pending = [];
    }
    for (const m of messages) {
      const summary = toolSummary(m, actionable);
      if (summary) {
        pending.push({
          id: m.id,
          text: summary,
          count: m.tool_calls!.filter((c) => !actionable.has(c.id)).length,
        });
      }
      // Calls whose proposal is already shown need no duplicate activity row.
      if (m.role === "assistant" && !m.content && !attached.has(m.id)) continue;
      flush();
      nodes.push(<Message key={m.id} m={m} {...msgProps} />);
    }
    flush();
    return nodes;
  }

  return (
    <div className="chat">
      <div className="chat-messages" ref={listRef} onScroll={onTranscriptScroll}>
        {!visible.length && sent === null && (
          <div className="chat-empty">
            <h3>Your deck conversation</h3>
            <p>Ask about your deck, explore a card, or work through a change. The agent proposes; you decide.</p>
          </div>
        )}
        {compacted.length > 0 && (
          <details className="compacted-block">
            <summary className="muted">
              {compacted.length} message(s) compacted out of context — still on disk
            </summary>
            {transcript(compacted)}
          </details>
        )}
        {transcript(resident)}
        {sent !== null && (
          <div className="chat-msg user">
            <AuditRefText text={sent} />
          </div>
        )}
        {busy && <div className="chat-tool muted">thinking…</div>}
      </div>
      {error && <div className="error-banner">{error}</div>}
      {/* The composer is one field: the form draws the border and focus ring,
          the textarea inside it is chromeless. */}
      <form onSubmit={send} className="chat-input">
        <textarea
          ref={boxRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={onComposerKey}
          aria-label="Message the deck agent"
          placeholder="Ask about your deck…"
          disabled={busy}
        />
        <button className="primary" type="submit" disabled={busy || !input.trim()}>
          Send
        </button>
      </form>
    </div>
  );
}
