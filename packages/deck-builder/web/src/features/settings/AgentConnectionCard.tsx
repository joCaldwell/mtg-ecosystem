import { useState } from "react";
import { api, type Settings } from "../../api/client.ts";
import { Box, Button, ConfirmDialog, ErrorNote, KeyValue, Row, Segmented, Stack, Text, useToast } from "../../ui/index.ts";
import styles from "./AgentConnectionCard.module.css";
import { clientConfigs, type ClientId } from "./clients.tsx";
import { CopyButton } from "./CopyButton.tsx";

/** What the server reports from the version that added the stdio shim; older servers omit it. */
type AgentInfo = Settings["agent"] & { stdio_command?: string };

const mask = (token: string) => `••••••••${token.slice(-4)}`;

/** How an agent reaches this app (spec §10.1): the endpoint, the token, and the paste-ready configuration for each client. */
export function AgentConnectionCard({ agent, onChanged }: { agent: AgentInfo; onChanged: () => void }) {
  const { toast } = useToast();
  const [revealed, setRevealed] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [clientId, setClientId] = useState<ClientId>("claude-code");

  const shown = clientConfigs(agent.url, revealed ? agent.token : mask(agent.token), agent.stdio_command);
  const real = clientConfigs(agent.url, agent.token, agent.stdio_command);
  const index = Math.max(0, shown.findIndex((c) => c.id === clientId));
  const client = shown[index];

  async function rotate() {
    setConfirming(false);
    setBusy(true);
    setError(null);
    try {
      await api.rotateToken();
      setRevealed(false);
      onChanged();
      toast("New token generated. Give it to each connected agent.");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Box>
      <Stack gap={5}>
        <Text as="h2" size="lg" weight="bold">
          Agent connection
        </Text>
        <Text tone="muted" size="sm">
          This app makes no AI calls itself. An agent you run elsewhere connects to it over MCP, on this machine only,
          and needs the token below. The name after <Text mono>/mcp/</Text> is yours to choose: it is how that agent's
          writes are labelled in Activity.
        </Text>

        <KeyValue
          items={[
            [
              "Endpoint",
              <Row key="url" gap={4}>
                <Text mono className={styles.value}>
                  {agent.url}
                </Text>
                <CopyButton text={agent.url} label="endpoint" />
              </Row>,
            ],
            [
              "Token",
              <Row key="token" gap={4}>
                <Text mono className={styles.value}>
                  {revealed ? agent.token : mask(agent.token)}
                </Text>
                <Button size="sm" onClick={() => setRevealed((r) => !r)} aria-pressed={revealed}>
                  {revealed ? "Hide" : "Show"}
                </Button>
                <CopyButton text={agent.token} label="token" />
                <Button size="sm" variant="ghost" onClick={() => setConfirming(true)} disabled={busy} aria-haspopup="dialog">
                  Regenerate
                </Button>
              </Row>,
            ],
            ["Reachable from", "This computer only"],
          ]}
        />
        {error && <ErrorNote>{error}</ErrorNote>}

        <Stack gap={4}>
          <Segmented
            label="Client"
            value={client.id}
            onChange={setClientId}
            options={shown.map((c) => ({ value: c.id, label: c.label }))}
          />
          {client.parts.map((part, i) =>
            "prose" in part ? (
              <Text key={i} tone="muted" size="sm" as="p">
                {part.prose}
              </Text>
            ) : (
              <div key={i} className={styles.snippet}>
                <pre className={styles.code} aria-label={part.label}>
                  {part.code}
                </pre>
                <div className={styles.copy}>
                  {/* Copy the real token even while the display is masked. */}
                  <CopyButton text={(real[index].parts[i] as { code: string }).code} label={part.label} />
                </div>
              </div>
            ),
          )}
        </Stack>
      </Stack>

      {confirming && (
        <ConfirmDialog
          title="Regenerate the token"
          tone="danger"
          confirmLabel="Regenerate token"
          onClose={() => setConfirming(false)}
          onConfirm={rotate}
        >
          <Stack gap={3}>
            <Text>Generate a new token?</Text>
            <Text tone="muted" size="sm">
              Every connected agent will need the new token. The current one stops working as soon as you confirm.
            </Text>
          </Stack>
        </ConfirmDialog>
      )}
    </Box>
  );
}
