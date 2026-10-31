import { useRef, useState } from "react";
import { api, type Settings } from "../../api/client.ts";
import { ago, parseServerTime } from "../../domain/time.ts";
import { Box, Button, ConfirmDialog, ErrorNote, Field, KeyValue, Row, Select, Stack, Text, TextField } from "../../ui/index.ts";
import styles from "./AgentConnectionCard.module.css";
import { clientConfigs, type ClientId, type Part } from "./clients.tsx";
import { CopyButton } from "./CopyButton.tsx";

/** Credentials stay in this component only until dismissed or the page is left. */
export function AgentConnectionCard({ agent, onChanged }: { agent: Settings["agent"]; onChanged: () => void }) {
  const [label, setLabel] = useState("");
  const [secret, setSecret] = useState<string | null>(null);
  const [revoking, setRevoking] = useState<number | null>(null);
  const [disconnecting,setDisconnecting] = useState<string|null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [clientId, setClientId] = useState<ClientId>("claude-code");
  const clients = clientConfigs(agent.url, secret ?? "<your agent token>", agent.stdio_command);
  const client = clients.find(c => c.id === clientId) ?? clients[0];
  const lastSeen = [...agent.tokens.filter(t => t.last_used_at && !t.revoked_at).map(t => ({ label: t.label, last_used_at: t.last_used_at! })), ...(agent.oauth_usage ?? [])]
    .sort((a, b) => parseServerTime(b.last_used_at) - parseServerTime(a.last_used_at))[0];
  // First connection needs the commands; established accounts keep them on demand.
  const [instructionsInitiallyOpen] = useState(!lastSeen);
  const instructions = useRef<HTMLDetailsElement>(null);
  function showInstructions() {
    if (instructions.current) instructions.current.open = true;
  }

  async function create() {
    setBusy(true); setError(null);
    try { const result = await api.createToken(label); setSecret(result.token); setLabel(""); showInstructions(); onChanged(); }
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
    <Text as="h2" size="lg" weight="bold">Connect an agent</Text>
    <KeyValue items={[
      ["Endpoint", <Row key="url" gap={4}><Text mono className={styles.value}>{agent.url}</Text><CopyButton text={agent.url} label="endpoint" /></Row>],
      ["Reachable from", agent.mode === "local" ? "This computer only" : "Your hosted address"],
    ]} />
    <div className={styles.columns}><Stack gap={5}>
    <Text as="h3" weight="bold">Agent tokens</Text>
    <Text tone="muted" size="sm">Tokens are shown once. Save yours before dismissing it.</Text>
    <Field label="Token label">
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
      {!token.revoked_at && <Button size="sm" variant="reject" disabled={busy} onClick={() => setRevoking(token.id)}>Revoke</Button>}
    </Row>)}
    {!agent.tokens.length && <Text tone="muted" size="sm">No agent tokens yet.</Text>}
    {agent.mode === "hosted" && <Stack gap={3}>
      <Text as="h3" weight="bold">Connected OAuth apps</Text>
      {(agent.oauth_clients ?? []).map(c=><Row key={c.client_id} gap={4} justify="between">
        <Stack gap={1} className={styles.value}><Text weight="medium">{c.label ?? "OAuth app"}</Text><Text size="sm" mono>{c.client_id}</Text><Text tone="muted" size="sm">{c.last_used_at ? `Last used ${ago(c.last_used_at)}` : "No MCP requests yet"}</Text></Stack>
        <Button size="sm" variant="reject" disabled={busy} onClick={()=>setDisconnecting(c.client_id)}>Disconnect</Button>
      </Row>)}
      {!agent.oauth_clients?.length && <Text size="sm" tone="muted">No OAuth apps connected.</Text>}
    </Stack>}
    </Stack><Stack gap={4}>
      <Text as="h3" weight="bold">Client setup</Text>
      <Text tone="muted" size="sm">{lastSeen ? `Last request: ${lastSeen.label}, ${ago(lastSeen.last_used_at)}` : "No authenticated MCP requests yet"}</Text>
      <Field label="Connection method"><Select options={clients.map(c => ({ value: c.id, label: c.label }))} value={client.id} onChange={value => { setClientId(value as ClientId); showInstructions(); }} /></Field>
      {/* The browser owns toggling; refreshes and queued toggle events must not
          overwrite an explicit close or a request to show a new method. */}
      <details ref={instructions} className={styles.instructions} open={instructionsInitiallyOpen}>
        <summary>Setup instructions</summary>
        <Stack gap={4}>{client.parts.map(renderPart)}</Stack>
      </details>
      {client.permissions && <details key={client.id} className={styles.permissions}>
        <summary>Tool permissions (optional)</summary>
        <Stack gap={4}>{client.permissions.map(renderPart)}</Stack>
      </details>}
    </Stack>
    </div>
  </Stack>
  {disconnecting !== null && <ConfirmDialog title="Disconnect OAuth app" tone="danger" confirmLabel="Disconnect" onClose={()=>setDisconnecting(null)} onConfirm={async()=>{
    setBusy(true);setError(null);
    try{await api.disconnectClient(disconnecting);setDisconnecting(null);onChanged();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}
  }}><Text>This removes its authorization and refresh tokens. Its current access tokens stop working immediately.</Text></ConfirmDialog>}
  {revoking != null && <ConfirmDialog title="Revoke agent token" tone="danger" confirmLabel="Revoke token" onClose={() => setRevoking(null)} onConfirm={revoke}>
    <Text>This client will lose access immediately. Other tokens keep working.</Text>
  </ConfirmDialog>}
  </Box>;
}

function renderPart(part: Part, i: number) {
  return "prose" in part ? <Text key={i} tone="muted" size="sm" as="p">{part.prose}</Text> :
    <div key={i} className={styles.snippet}><pre className={styles.code} aria-label={part.label}>{part.code}</pre><div className={styles.copy}><CopyButton text={part.code} label={part.label} /></div></div>;
}
