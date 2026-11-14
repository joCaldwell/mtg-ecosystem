import { useState } from "react";
import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { Box, Button, ErrorNote, Row, Stack, Text } from "../../ui/index.ts";
import { CopyButton } from "./CopyButton.tsx";
export function AdminCard() {
  const query=useQuery(()=>api.admin(),[],"jobs"),[error,setError]=useState<string|null>(null),[busy,setBusy]=useState(false);
  async function run(action:()=>Promise<unknown>) {setBusy(true);setError(null);try{await action();query.reload();}catch(e){setError((e as Error).message);}finally{setBusy(false);}}
  return <Box><Stack gap={4}><Text as="h2" size="lg" weight="bold">Admin</Text>
    {query.data && <><Text>{query.data.users} users · {(query.data.database_bytes/1024/1024).toFixed(1)} MiB database</Text>
      <Text tone={query.data.jobs?.backup.stale ? "cut" : "muted"}>Last successful backup: {query.data.jobs?.backup.last_success_at ?? "never"}{query.data.jobs?.backup.stale ? " · stale (over 36 hours or never completed)" : ""}</Text>
      {(["refresh","backup"] as const).map(job=><Row key={job} gap={3} wrap><Text>{job}: {query.data!.jobs?.[job].result ?? "not run"} · last {query.data!.jobs?.[job].last_run ?? "never"} · next {query.data!.jobs ? new Date(query.data!.jobs[job].next_run).toLocaleString() : "not scheduled"}</Text><Button size="sm" disabled={busy || !!query.data!.jobs?.active} onClick={()=>run(()=>api.enqueueJob(job))}>Enqueue {job}</Button></Row>)}
      <Row justify="between"><Text as="h3" weight="bold">Invite codes</Text><Button disabled={busy} onClick={()=>run(()=>api.createInvite())}>Create invite</Button></Row>
      {query.data.invites.map(i=><Row gap={3} key={i.code}><Text mono>{i.used_at ? "Used invite" : i.code}</Text><Text tone="muted" size="sm">{i.used_at ? `Used ${i.used_at}` : "Unused"}</Text>{!i.used_at && <CopyButton text={i.code} label="invite" />}</Row>)}
    </>}{(error || query.error) && <ErrorNote>{error || query.error}</ErrorNote>}
  </Stack></Box>;
}
