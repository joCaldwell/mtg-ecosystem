// The configuration an owner pastes into each kind of agent client. Ported
// from the previous Settings page, whose snippets were written against each
// client's own documentation; do not alter the syntax without re-checking it.

import type { ReactNode } from "react";

export type ClientId = "claude-code" | "codex" | "chatgpt" | "http" | "stdio";

export type Part = { prose: ReactNode } | { code: string; label: string };

export interface Client {
  id: ClientId;
  label: string;
  parts: Part[];
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
      label: "Claude Code",
      parts: [
        {
          label: "Claude Code command",
          code: `claude mcp add --transport http deck-builder ${url}/claude-code --header "Authorization: Bearer ${token}"`,
        },
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
      label: "Codex",
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

  if (stdio) {
    clients.push({
      id: "chatgpt",
      label: "ChatGPT (web, phone)",
      parts: [
        {
          prose: (
            <>
              ChatGPT's servers cannot reach an app that only listens on this machine. OpenAI's Secure MCP Tunnel is
              the supported way through: a small program you run here that connects out to OpenAI, so nothing is
              opened to the internet. Run these commands from the deck-builder package directory. It needs an OpenAI Platform organization with Tunnels enabled, a tunnel id and a
              runtime API key from <Code>platform.openai.com/settings/organization/tunnels</Code>, and the{" "}
              <Code>tunnel-client</Code> program from there. These commands follow OpenAI's guide and have not been
              run against this app; what you say to ChatGPT about your decks then passes through OpenAI, as anything
              said to ChatGPT does.
            </>
          ),
        },
        {
          label: "tunnel-client commands",
          code: [
            'export CONTROL_PLANE_API_KEY="sk-..."',
            "tunnel-client init \\",
            "  --sample sample_mcp_stdio_local \\",
            "  --profile deck-builder \\",
            "  --tunnel-id <your tunnel id> \\",
            `  --mcp-command "${stdio} --name chatgpt"`,
            "tunnel-client doctor --profile deck-builder --explain",
            "tunnel-client run --profile deck-builder",
          ].join("\n"),
        },
        {
          prose: (
            <>
              Then in ChatGPT, open Settings → Connectors, choose <strong>Connection: Tunnel</strong> and pick the
              tunnel. Keep <Code>tunnel-client run</Code> and this app running while you use it.
            </>
          ),
        },
      ],
    });
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
              started first. Run this command from the deck-builder package directory. It reads the token from the app's data folder.
            </>
          ),
        },
        { label: "stdio command", code: `${stdio} --name my-agent` },
      ],
    });
  }

  return clients;
}
