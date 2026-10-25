import { useState, type ReactNode } from "react";
import { ago } from "../../domain/time.ts";
import { Box, Button, Chip, Row, Section, Stack, Text } from "../../ui/index.ts";

/** Audit summaries stay scannable; the stored prose is available on demand. */
export function HistoryEntry({ title, status, subject, author, createdAt, basis, children }: {
  title: string; status: string; subject: string; author: string; createdAt: string;
  basis: string; children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  return <Section title={title} aside={<Row><Chip>{status}</Chip><Button size="sm" variant="ghost" aria-expanded={open} onClick={() => setOpen(!open)}>{open ? "Hide details" : "Show details"}</Button></Row>}>
    <Box><Stack gap={3}>
      <Row wrap><Text weight="bold">{subject}</Text><Text tone="muted">{author} · {ago(createdAt)}</Text></Row>
      {basis && <Text tone="faint">{basis}</Text>}
      {open && children}
    </Stack></Box>
  </Section>;
}
