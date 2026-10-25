import { useState } from "react";
import { api, type Settings } from "../../api/client.ts";
import { ago } from "../../domain/time.ts";
import { Box, Button, ConfirmDialog, ErrorNote, Field, KeyValue, Row, Select, Stack, Text, TextField } from "../../ui/index.ts";
import styles from "./AgentConnectionCard.module.css";
import { clientConfigs, type ClientId } from "./clients.tsx";
import { CopyButton } from "./CopyButton.tsx";

/** Credentials stay in this component only until dismissed or the page is left. */
export function AgentConnectionCard({ agent, onChanged }: { agent: Settings["agent"]; onChanged: () => void }) {
  const [label, setLabel] = useState("");
  const [secret, setSecret] = useState<string | null>(null);
  const [revoking, setRevoking] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [clientId, setClientId] = useState<ClientId>("claude-code");
  const clients = clientConfigs(agent.url, secret ?? "<your agent token>", agent.stdio_command);
  const client = clients.find(c => c.id === clientId) ?? clients[0];

  async function create() {
    setBusy(true); setError(null);
    try { const result = await api.createToken(label); setSecret(result.token); setLabel(""); onChanged(); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  }
  async function revoke() {
    if (revoking == null) return;
    setBusy(true); setError(null);
    try { await api.revokeToken(revoking); setSecret(null); setRevoking(null); onChanged(); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  }

  return <Box><Stack gap={5}>
    <Text as="h2" size="lg" weight="bold">Agent connection</Text>
    <Text tone="muted" size="sm">
      This app makes no AI calls itself. Each agent connects over MCP with a token for your account.
      The name after <Text mono>/mcp/</Text> labels its writes in Activity.
    </Text>
    <KeyValue items={[
      ["Endpoint", <Row key="url" gap={4}><Text mono className={styles.value}>{agent.url}</Text><CopyButton text={agent.url} label="endpoint" /></Row>],
      ["Reachable from", agent.mode === "local" ? "This computer only" : "Your hosted address"],
    ]} />
    <Text as="h3" weight="bold">Agent tokens</Text>
    <Text tone="muted" size="sm">Create a token for each client. Save it when shown; it cannot be revealed again.</Text>
    <Field label="Token label" hint="For example, the client or computer you connect from.">
      <TextField value={label} maxLength={80} onChange={e => setLabel(e.target.value)} />
    </Field>
    <Row gap={4}><Button variant="primary" disabled={busy || !label.trim() || secret !== null} onClick={create}>{busy ? "Working…" : "Create token"}</Button></Row>
    {error && <ErrorNote>{error}</ErrorNote>}
    {secret && <Stack gap={3}>
      <Text weight="bold">Copy and save this token now</Text>
      <Text mono className={styles.value}>{secret}</Text>
      <Row gap={4}><CopyButton text={secret} label="new token" /><Button size="sm" onClick={() => setSecret(null)}>I saved it</Button></Row>
    </Stack>}
    {agent.tokens.map(token => <Row key={token.id} gap={4} justify="between">
      <Stack gap={1} className={styles.value}><Text weight="medium">{token.label}</Text><Text tone="muted" size="sm">{token.prefix}… · {token.revoked_at ? "Revoked" : token.last_used_at ? `Last used ${ago(token.last_used_at)}` : "Never used"}</Text></Stack>
      {!token.revoked_at && <Button size="sm" variant="ghost" disabled={busy} onClick={() => setRevoking(token.id)}>Revoke</Button>}
    </Row>)}
    {!agent.tokens.length && <Text tone="muted" size="sm">No agent tokens yet.</Text>}
    <Stack gap={4}>
      <Field label="Client"><Select value={client.id} onChange={e => setClientId(e.target.value as ClientId)}>{clients.map(c => <option key={c.id} value={c.id}>{c.label}</option>)}</Select></Field>
      {client.parts.map((part, i) => "prose" in part ? <Text key={i} tone="muted" size="sm" as="p">{part.prose}</Text> :
        <div key={i} className={styles.snippet}><pre className={styles.code} aria-label={part.label}>{part.code}</pre><div className={styles.copy}><CopyButton text={part.code} label={part.label} /></div></div>)}
    </Stack>
  </Stack>
  {revoking != null && <ConfirmDialog title="Revoke agent token" tone="danger" confirmLabel="Revoke token" onClose={() => setRevoking(null)} onConfirm={revoke}>
    <Text>This client will lose access immediately. Other tokens keep working.</Text>
  </ConfirmDialog>}
  </Box>;
}
