import { useState } from "react";
import { api } from "../../api/client.ts";
import { useSession } from "../../app/Session.tsx";
import { Box, Button, Dialog, ErrorNote, Field, Row, Stack, Text, TextField } from "../../ui/index.ts";
export function AccountCard() {
  const {session} = useSession(), user=session.user!;
  const [confirm,setConfirm]=useState(false),[handle,setHandle]=useState(""),[error,setError]=useState<string|null>(null),[busy,setBusy]=useState(false);
  async function download() {
    setBusy(true); setError(null);
    try {
      const data=await api.exportAccount(), url=URL.createObjectURL(new Blob([JSON.stringify(data,null,2)],{type:"application/json"}));
      const a=document.createElement("a"); a.href=url;a.download=`deck-builder-${user.handle}.json`;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
    } catch(e){setError((e as Error).message);} finally{setBusy(false);}
  }
  async function remove() {setBusy(true);try{await api.deleteAccount(handle);location.assign("/signin");}catch(e){setError((e as Error).message);}finally{setBusy(false);}}
  async function signOut(){try{await api.signOut();location.assign("/signin");}catch(e){setError((e as Error).message);}}
  return <Box><Stack gap={4}>
    <Text as="h2" size="lg" weight="bold">Account · @{user.handle}</Text>
    <Text tone="muted">{user.display_name}</Text>
    <Row gap={3} wrap><Button onClick={download} disabled={busy}>Export all my data</Button><Button onClick={signOut} disabled={busy}>Sign out</Button><Button variant="reject" disabled={!user.can_delete || busy} onClick={()=>setConfirm(true)}>Delete account</Button></Row>
    {user.deletion_reason && <Text tone="muted" size="sm">{user.deletion_reason}</Text>}{error && <ErrorNote>{error}</ErrorNote>}
  </Stack>{confirm && <Dialog title="Permanently delete your account" onClose={()=>setConfirm(false)} footer={<><Button onClick={()=>setConfirm(false)}>Cancel</Button><Button variant="reject" onClick={remove} disabled={handle !== user.handle || busy}>Delete account</Button></>}>
    <Stack gap={4}><Text>All decks, memories, proposals, activity and credentials will be removed. Export first if you want a copy.</Text><Field label={`Type ${user.handle} to confirm`}><TextField value={handle} onChange={e=>setHandle(e.target.value)} /></Field></Stack>
  </Dialog>}</Box>;
}
