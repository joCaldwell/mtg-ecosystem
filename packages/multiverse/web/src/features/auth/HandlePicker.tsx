import { useWindowScroll } from "../../app/windowScroll.ts";
import styles from "./StandalonePage.module.css";
import { useState, type FormEvent } from "react";
import { api } from "../../api/client.ts";
import { useSession } from "../../app/Session.tsx";
import { Box, Button, ErrorNote, Field, Page, PageHeader, Stack, Text, TextField } from "../../ui/index.ts";

export function HandlePicker() {
  useWindowScroll();
  const { session, reload } = useSession();
  const [handle, setHandle] = useState(session.user?.handle ?? "");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function save(event: FormEvent) {
    event.preventDefault();
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await api.chooseHandle(handle);
      reload();
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  return <main id="main-content" tabIndex={-1} className={styles.page}><Page width="md" header={<PageHeader title="Choose your handle" />}>
    <Box><form method="post" onSubmit={save} aria-label="Choose your handle"><Stack gap={4}>
      <Text tone="muted">Your provider name was already taken. We reserved a unique starting point.</Text>
      <Field label="Handle" hint="3–24 lowercase letters, digits, underscores or hyphens.">
        <TextField name="handle" autoComplete="username" autoCapitalize="none" autoCorrect="off" spellCheck={false}
          required minLength={3} maxLength={24} pattern="[a-z0-9_\-]{3,24}" value={handle} disabled={busy}
          onChange={event => { setHandle(event.target.value); setError(null); }} />
      </Field>
      {error && <ErrorNote>{error}</ErrorNote>}
      <Button type="submit" variant="primary" disabled={busy}>{busy ? "Saving…" : "Save handle"}</Button>
    </Stack></form></Box>
  </Page></main>;
}
