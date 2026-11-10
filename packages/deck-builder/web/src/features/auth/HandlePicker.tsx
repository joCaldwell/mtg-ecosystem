import { useState } from "react";
import { api } from "../../api/client.ts";
import { useSession } from "../../app/Session.tsx";
import { Box, Button, ErrorNote, Field, Page, PageHeader, Stack, Text, TextField } from "../../ui/index.ts";
export function HandlePicker() {
  const {session,reload} = useSession();
  const [handle,setHandle] = useState(session.user?.handle ?? ""),[error,setError] = useState<string|null>(null),[busy,setBusy]=useState(false);
  async function save() { setBusy(true); try { await api.chooseHandle(handle); reload(); } catch(e) {setError((e as Error).message);} finally {setBusy(false);} }
  return <Page width="md" header={<PageHeader title="Choose your handle" />}><Box><Stack gap={4}>
    <Text tone="muted">Your provider name was already taken. We reserved a unique starting point.</Text>
    <Field label="Handle" hint="3–24 lowercase letters, digits, underscores or hyphens."><TextField value={handle} maxLength={24} onChange={e=>setHandle(e.target.value)} /></Field>
    {error && <ErrorNote>{error}</ErrorNote>}<Button variant="primary" disabled={busy || !/^[a-z0-9_-]{3,24}$/.test(handle)} onClick={save}>Save handle</Button>
  </Stack></Box></Page>;
}
