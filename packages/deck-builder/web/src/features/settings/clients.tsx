// The configuration an owner pastes into each kind of agent client. Ported
// from the previous Settings page, whose snippets were written against each
// client's own documentation; do not alter the syntax without re-checking it.

import type { ReactNode } from "react";

export type ClientId = "claude-code" | "claude-code-oauth" | "codex" | "chatgpt" | "claude-ai" | "http" | "stdio";

export type Part = { prose: ReactNode } | { code: string; label: string };

export interface Client {
  id: ClientId;
  label: string;
  parts: Part[];
  permissions?: Part[];
}

const Code = ({ children }: { children: ReactNode }) => <code>{children}</code>;

/**
 * `token` is whatever should appear in the snippet: the real one to copy, or a
 * masked one to show. `stdio` is the command that pipes to the running app;
 * the server only reports it from the version that added the stdio shim, so
 * the two clients that need it are left out when it is missing.
 */
export function clientConfigs(url: string, token: string, stdio: string | undefined): Client[] {
  const clients: Client[] = [
    {
      id: "claude-code",
      label: "Claude Code · personal token",
      parts: [
        {
          label: "Claude Code command",
          // Keep the bearer out of argv/history/config. The client expands the
          // literal environment reference when it connects (vendor docs in ops).
          code: [
            "printf 'Paste the token from Settings: '",
            "read -rs DECKBUILDER_TOKEN",
            "printf '\\n'",
            "export DECKBUILDER_TOKEN",
            `claude mcp add-json deck-builder '${JSON.stringify({ type: "http", url: `${url}/claude-code`, headers: { Authorization: "Bearer ${DECKBUILDER_TOKEN}" } })}'`,
            "# Launch Claude Code from this environment; keep the variable set.",
          ].join("\n"),
        },
      ],
      permissions: [
        {
          prose: (
            <>
              Claude Code asks before each tool call until the server is allowed. To let it read and write here
              without asking, add this to <Code>~/.claude/settings.json</Code>:
            </>
          ),
        },
        {
          label: "Claude Code permission",
          code: JSON.stringify({ permissions: { allow: ["mcp__deck-builder"] } }, null, 2),
        },
      ],
    },
    {
      id: "codex",
      label: "Codex · personal token",
      parts: [
        {
          prose: (
            <>
              The Codex CLI, desktop app and IDE extension share one file. Add this to{" "}
              <Code>~/.codex/config.toml</Code> and restart Codex. <Code>approve</Code> lets it read and write here
              without asking; use <Code>writes</Code> to be asked before anything that writes.
            </>
          ),
        },
        {
          label: "Codex configuration",
          code: [
            "[mcp_servers.deck-builder]",
            `url = "${url}/codex"`,
            `http_headers = { Authorization = "Bearer ${token}" }`,
            'default_tools_approval_mode = "approve"',
          ].join("\n"),
        },
      ],
    },
  ];

  if (!stdio) {
    // Verified 2026-10-25 against code.claude.com/docs/en/mcp and
    // developers.openai.com/api/docs/mcp (remote connectors use OAuth).
    clients.splice(1, 0, { id: "claude-code-oauth", label: "Claude Code · OAuth", parts: [
      { prose: <>Run this without a token. Open Claude Code’s /mcp menu and choose Authenticate, then sign in and approve deck-builder access.</> },
      { label: "Claude Code OAuth", code: `claude mcp add --transport http deck-builder-oauth ${url}` },
    ], permissions: clients[0].permissions });
    clients.push({ id: "claude-ai", label: "Claude.ai · OAuth", parts: [
      { prose: <>In Claude.ai, open Settings → Connectors → Add custom connector. Enter this hosted URL, then choose Connect. Sign in with your deck-builder account and approve access. A public HTTPS address is required.</> },
      { label: "Claude.ai connector URL", code: url },
    ] }, { id: "chatgpt", label: "ChatGPT · OAuth", parts: [
      { prose: <>In ChatGPT, enable developer mode in Settings → Apps, then create an app with this MCP URL and OAuth authentication. Connect, sign in and approve deck-builder access. Your plan or workspace must allow custom apps; a public HTTPS address is required.</> },
      { label: "ChatGPT MCP URL", code: url },
    ] });
  }

  clients.push({
    id: "http",
    label: "Other (URL and headers)",
    parts: [
      { prose: <>Any client that takes a URL and headers.</> },
      {
        label: "HTTP configuration",
        code: JSON.stringify(
          {
            mcpServers: {
              "deck-builder": { type: "http", url: `${url}/my-agent`, headers: { Authorization: `Bearer ${token}` } },
            },
          },
          null,
          2,
        ),
      },
    ],
  });

  if (stdio) {
    clients.push({
      id: "stdio",
      label: "Other (stdio)",
      parts: [
        {
          prose: (
            <>
              For a client that only launches a command. The command is a pipe to this running app, so the app must be
              started first. In local mode it reads data/stdio-token, preserved from the previous token on upgrade. For a new token, save it in a private file and add --token-file /private/path/to/token (or set DECKBUILDER_TOKEN_FILE).
            </>
          ),
        },
        { label: "stdio command", code: `${stdio} --name my-agent` },
      ],
    });
  }

  return clients;
}
