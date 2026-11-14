import { useEffect, useId, useRef, useState, type FormEvent } from "react";
import { api, type Settings } from "../../api/client.ts";
import { Box, Button, ErrorNote, Field, Row, Stack, Table, Text, TextField } from "../../ui/index.ts";
import styles from "./BudgetsCard.module.css";

// The server's own bounds (setBudgets in memory.ts); stated here so a bad value is caught before the request.
const MIN = 100;
const MAX = 20000;

/** How much memory an agent is handed without asking (spec §7.4), in estimated tokens. */
export function BudgetsCard({ budgets, usage, onSaved }: { budgets: Settings["budgets"]; usage: Settings["memory_usage"]; onSaved: () => void }) {
  const headingId = useId();
  const previous = useRef(budgets);
  const [global, setGlobal] = useState(String(budgets.global));
  const [deck, setDeck] = useState(String(budgets.deck));
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Follow the server when it changes (another tab, an agent), unless the fields are being edited.
  const dirty = global !== String(budgets.global) || deck !== String(budgets.deck);
  useEffect(() => {
    const before = previous.current;
    setGlobal(current => current === String(before.global) ? String(budgets.global) : current);
    setDeck(current => current === String(before.deck) ? String(budgets.deck) : current);
    previous.current = budgets;
  }, [budgets.global, budgets.deck]);

  async function save(event: FormEvent) {
    event.preventDefault();
    if (busy || !dirty) return;
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
      <form method="post" onSubmit={save} aria-labelledby={headingId}><Stack gap={5}>
        <Text as="h2" id={headingId} size="lg" weight="bold">
          Memory budgets
        </Text>
        <Text tone="muted" size="sm">
          Estimated tokens; newest memories load first. Omitted notes stay searchable.
        </Text>
        <div className={styles.fields}>
          <Field label="Global limit (tokens)" hint={`Whole number from ${MIN.toLocaleString()} to ${MAX.toLocaleString()}.`}>
            <TextField
              type="number" min={MIN} max={MAX} step={1} required disabled={busy}
              inputMode="numeric"
              value={global}
              name="global"
              onChange={(e) => {
                setGlobal(e.target.value);
                setSaved(false);
              }}
            />
          </Field>
          <Field label="Per-deck limit (tokens)" hint={`Whole number from ${MIN.toLocaleString()} to ${MAX.toLocaleString()}.`}>
            <TextField
              type="number" min={MIN} max={MAX} step={1} required disabled={busy}
              inputMode="numeric"
              value={deck}
              name="deck"
              onChange={(e) => {
                setDeck(e.target.value);
                setSaved(false);
              }}
            />
          </Field>
        </div>
        {error && <ErrorNote>{error}</ErrorNote>}
        <Row gap={4}>
          <Button type="submit" variant="primary" disabled={busy || !dirty}>
            {busy ? "Saving…" : "Save"}
          </Button>
          {saved && !dirty && (
            <Text tone="add" size="sm" role="status">
              Saved.
            </Text>
          )}
        </Row>
        <details className={styles.usage}>
          <summary>Memory usage by deck</summary>
          <Table aria-label="Memory usage">
            <thead><tr><th scope="col">Scope</th><th scope="col">Loaded / limit</th><th scope="col">Omitted</th></tr></thead>
            <tbody>{usage.map(scope => <tr key={scope.deck_id ?? "global"}>
              <th scope="row">{scope.name}</th>
              <td><Text mono size="sm">{scope.used.toLocaleString()} / {scope.budget.toLocaleString()}</Text></td>
              <td><Text mono size="sm" tone={scope.omitted > 0 ? "warn" : "muted"}>{scope.omitted}</Text></td>
            </tr>)}</tbody>
          </Table>
        </details>
      </Stack></form>
    </Box>
  );
}
