import { useEffect, useState } from "react";
import { api, type Settings } from "../../api/client.ts";
import { Box, Button, ErrorNote, Field, Row, Stack, Table, Text, TextField } from "../../ui/index.ts";
import styles from "./BudgetsCard.module.css";

// The server's own bounds (setBudgets in memory.ts); stated here so a bad value is caught before the request.
const MIN = 100;
const MAX = 20000;

const valid = (s: string) => /^\d+$/.test(s.trim()) && Number(s) >= MIN && Number(s) <= MAX;

/** How much memory an agent is handed without asking (spec §7.4), in estimated tokens. */
export function BudgetsCard({ budgets, usage, onSaved }: { budgets: Settings["budgets"]; usage: Settings["memory_usage"]; onSaved: () => void }) {
  const [global, setGlobal] = useState(String(budgets.global));
  const [deck, setDeck] = useState(String(budgets.deck));
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Follow the server when it changes (another tab, an agent), unless the fields are being edited.
  const dirty = global !== String(budgets.global) || deck !== String(budgets.deck);
  useEffect(() => {
    if (!dirty) {
      setGlobal(String(budgets.global));
      setDeck(String(budgets.deck));
    }
    // `dirty` is derived from the values below; only a change on the server should reset the fields.
  }, [budgets.global, budgets.deck]);

  const ok = valid(global) && valid(deck);

  async function save() {
    setBusy(true);
    setError(null);
    try {
      await api.updateSettings({ budgets: { global: Number(global), deck: Number(deck) } });
      setSaved(true);
      onSaved();
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
          Memory budgets
        </Text>
        <Text tone="muted" size="sm">
          Newest memories load first. Omitted notes stay searchable.
        </Text>
        <Table aria-label="Memory usage">
          <thead><tr><th scope="col">Scope</th><th scope="col">Tokens loaded / limit</th><th scope="col">Omitted</th></tr></thead>
          <tbody>{usage.map(scope => <tr key={scope.deck_id ?? "global"}>
            <th scope="row">{scope.name}</th>
            <td><Text mono size="sm">{scope.used.toLocaleString()} / {scope.budget.toLocaleString()}</Text></td>
            <td><Text mono size="sm" tone={scope.omitted > 0 ? "warn" : "muted"}>{scope.omitted}</Text></td>
          </tr>)}</tbody>
        </Table>
        <div className={styles.fields}>
          <Field label="Global memories" hint="Estimated tokens loaded at session start.">
            <TextField
              inputMode="numeric"
              value={global}
              invalid={!valid(global)}
              onChange={(e) => {
                setGlobal(e.target.value);
                setSaved(false);
              }}
            />
          </Field>
          <Field label="Per deck" hint="Estimated tokens loaded with each deck.">
            <TextField
              inputMode="numeric"
              value={deck}
              invalid={!valid(deck)}
              onChange={(e) => {
                setDeck(e.target.value);
                setSaved(false);
              }}
            />
          </Field>
        </div>
        {!ok && (
          <Text tone="cut" size="sm" role="alert">
            Each budget is a whole number between {MIN.toLocaleString()} and {MAX.toLocaleString()}.
          </Text>
        )}
        {error && <ErrorNote>{error}</ErrorNote>}
        <Row gap={4}>
          <Button variant="primary" onClick={save} disabled={busy || !dirty || !ok}>
            {busy ? "Saving…" : "Save"}
          </Button>
          {saved && !dirty && (
            <Text tone="add" size="sm">
              Saved.
            </Text>
          )}
        </Row>
      </Stack>
    </Box>
  );
}
