import { useEffect, useState } from "react";
import { api, type Settings } from "./api.ts";
import { ConfirmDialog } from "./ConfirmDialog.tsx";

function Copyable({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="copyable">
      <pre className="oracle-text">{text}</pre>
      <button
        className="small"
        onClick={() =>
          navigator.clipboard.writeText(text).then(
            () => {
              setCopied(true);
              window.setTimeout(() => setCopied(false), 1500);
            },
            () => {},
          )
        }
        aria-label={`Copy ${label}`}
      >
        {copied ? "copied" : "copy"}
      </button>
    </div>
  );
}

/** Connection details for agents, memory budgets, and card data (spec §11). */
export function SettingsPage() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmRotate, setConfirmRotate] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [budgets, setBudgets] = useState<{ global: string; deck: string } | null>(null);

  useEffect(() => {
    api.getSettings().then(setSettings).catch((e) => setError(e.message));
  }, []);
  useEffect(() => {
    if (settings) setBudgets({ global: String(settings.budgets.global), deck: String(settings.budgets.deck) });
  }, [settings]);

  if (!settings || !budgets) return <div className="page muted">{error ?? "Loading…"}</div>;
  const { url, token } = settings.agent;

  const act = async (fn: () => Promise<Settings>) => {
    setError(null);
    try {
      setSettings(await fn());
    } catch (e: any) {
      setError(e.message);
    }
  };

  return (
    <div className="page">
      <header className="deck-list-heading">
        <h1>Settings</h1>
      </header>
      {error && <div className="error-banner">{error}</div>}

      <section className="settings-section">
        <h2>Connect an agent</h2>
        <p className="muted">
          This app makes no AI calls itself. An agent you run elsewhere connects to it over MCP, on this
          machine only, and needs the token below. The name after <code>/mcp/</code> is yours to choose:
          it is how that agent's writes are labelled in Activity.
        </p>
        <h3>Claude Code</h3>
        <Copyable
          label="Claude Code command"
          text={`claude mcp add --transport http deck-builder ${url}/claude-code --header "Authorization: Bearer ${token}"`}
        />
        <p className="muted">
          Claude Code asks before each tool call until the server is allowed. To let it read and write
          here without asking, add this to <code>~/.claude/settings.json</code>:
        </p>
        <Copyable
          label="Claude Code permission"
          text={JSON.stringify({ permissions: { allow: ["mcp__deck-builder"] } }, null, 2)}
        />
        <h3>Any client that takes a URL and headers</h3>
        <Copyable
          label="HTTP configuration"
          text={JSON.stringify(
            { mcpServers: { "deck-builder": { type: "http", url: `${url}/my-agent`, headers: { Authorization: `Bearer ${token}` } } } },
            null,
            2,
          )}
        />
        <h3>A client that only launches a command (stdio)</h3>
        <p className="muted">
          Run from <code>packages/deck-builder</code>. The command is a pipe to this running app, so the
          app must be started first. It reads the token from the app's data folder.
        </p>
        <Copyable label="stdio command" text="node src/mcp/stdio.ts --name my-agent" />
        <h3>Token</h3>
        <Copyable label="token" text={token} />
        <button className="small danger" onClick={() => setConfirmRotate(true)} aria-haspopup="dialog">
          Replace the token
        </button>
      </section>

      <section className="settings-section">
        <h2>Memory budgets</h2>
        <p className="muted">
          How much memory an agent is shown without asking, in estimated tokens: global memories at the
          start of a session, a deck's memories when it opens that deck. When a scope outgrows its
          budget the newest load, and the agent is told to consolidate.
        </p>
        <div className="row gap wrap">
          <label className="field">
            Global
            <input value={budgets.global} onChange={(e) => setBudgets({ ...budgets, global: e.target.value })} inputMode="numeric" size={6} />
          </label>
          <label className="field">
            Per deck
            <input value={budgets.deck} onChange={(e) => setBudgets({ ...budgets, deck: e.target.value })} inputMode="numeric" size={6} />
          </label>
          <button
            className="primary"
            disabled={budgets.global === String(settings.budgets.global) && budgets.deck === String(settings.budgets.deck)}
            onClick={() => act(() => api.updateSettings({ budgets: { global: Number(budgets.global), deck: Number(budgets.deck) } }))}
          >
            Save
          </button>
        </div>
      </section>

      <section className="settings-section">
        <h2>Card data</h2>
        <p className="muted">
          {settings.cards.count.toLocaleString()} cards from Scryfall's oracle-cards bulk file
          {settings.cards.updated_at ? `, loaded ${settings.cards.updated_at.slice(0, 10)}` : ""}.
        </p>
        <button
          disabled={refreshing}
          onClick={async () => {
            setRefreshing(true);
            await act(() => api.refreshCards());
            setRefreshing(false);
          }}
        >
          {refreshing ? "Downloading and loading…" : "Download the latest card data"}
        </button>
      </section>

      {confirmRotate && (
        <ConfirmDialog
          title="Replace the token"
          confirmLabel="Replace token"
          destructive
          onClose={() => setConfirmRotate(false)}
          onConfirm={() => {
            setConfirmRotate(false);
            void act(() => api.rotateToken());
          }}
        >
          <p>Generate a new token?</p>
          <p className="muted">Every agent configured with the current one stops working until you give it the new one.</p>
        </ConfirmDialog>
      )}
    </div>
  );
}
