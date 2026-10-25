// What a reader can do with a shared deck (spec §18.5): take its list as
// Archidekt text (no account needed), fork it into a deck of their own, or
// compare it with one of their decks. Fork and compare need a session.

import { useEffect, useId, useState, type FormEvent } from "react";
import { api, type DeckComparison, type PublicDeckState } from "../../api/client.ts";
import { useIsPhone } from "../../app/breakpoints.ts";
import { navigate } from "../../app/routes.ts";
import { DeckDifferences } from "../deck/index.ts";
import { Button, Checkbox, Dialog, ErrorNote, Field, Loading, Select, Stack, Text, TextArea, TextField, useToast } from "../../ui/index.ts";

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function ExportSharedDialog({ shared, onClose }: { shared: PublicDeckState; onClose: () => void }) {
  const phone = useIsPhone();
  const { toast } = useToast();
  const [categories, setCategories] = useState(true);
  const [maybe, setMaybe] = useState(false);
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let current = true;
    setText(null);
    api.publicExport(shared.deck.slug, { categories, includeMaybe: maybe }).then(
      (r) => current && setText(r.text), (e) => current && setError(message(e)));
    return () => { current = false; };
  }, [shared.deck.slug, categories, maybe]);
  async function copy() {
    if (text == null) return;
    try { await navigator.clipboard.writeText(text); toast("List copied."); } catch { toast("Select the text and copy it."); }
  }
  return (
    <Dialog title="Export this list" onClose={onClose} placement={phone ? "sheet" : "center"}
      footer={<><Button size={phone ? "touch" : "md"} variant="ghost" onClick={onClose}>Close</Button><Button size={phone ? "touch" : "md"} variant="primary" disabled={text == null} onClick={() => void copy()}>Copy</Button></>}>
      <Stack gap={5}>
        <Text tone="muted" as="p">Archidekt's text format, which most deck sites import. The command zone comes first.</Text>
        <Checkbox checked={categories} onChange={setCategories} label="Categories from the owner's slots" />
        {shared.computed.maybe_count > 0 && <Checkbox checked={maybe} onChange={setMaybe} label="Include the maybe list" />}
        {error && <ErrorNote>{error}</ErrorNote>}
        {text == null && !error ? <Loading /> : <TextArea readOnly rows={12} value={text ?? ""} aria-label="The list as text" onFocus={(e) => e.currentTarget.select()} />}
      </Stack>
    </Dialog>
  );
}

export function ForkSharedDialog({ shared, onClose }: { shared: PublicDeckState; onClose: () => void }) {
  const phone = useIsPhone();
  const formId = useId();
  const [name, setName] = useState(`${shared.deck.name} (fork)`);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!name.trim() || busy) return;
    setBusy(true); setError(null);
    try {
      const result = await api.forkShared(shared.deck.slug, name.trim());
      onClose();
      navigate({ page: "deck", deckId: result.state.deck.id });
    } catch (err) { setError(message(err)); } finally { setBusy(false); }
  }
  return (
    <Dialog title="Fork this deck" onClose={onClose} placement={phone ? "sheet" : "center"}
      footer={<><Button size={phone ? "touch" : "md"} variant="ghost" onClick={onClose}>Cancel</Button><Button size={phone ? "touch" : "md"} variant="primary" type="submit" form={formId} disabled={!name.trim() || busy}>{busy ? "Forking…" : "Fork"}</Button></>}>
      <form id={formId} onSubmit={submit}>
        <Stack gap={5}>
          <Text tone="muted" as="p">
            Makes a private Theorycrafted deck of your own with this list, its slots, tags, brief and engines. Nothing of
            {" "}{shared.deck.owner.handle}'s notes, decisions or power records comes with it. It remembers where it came from.
          </Text>
          <Field label="Name for your fork">
            <TextField value={name} onChange={(e) => setName(e.target.value)} onFocus={(e) => e.currentTarget.select()} data-autofocus="" />
          </Field>
          {error && <ErrorNote>{error}</ErrorNote>}
        </Stack>
      </form>
    </Dialog>
  );
}

export function CompareSharedDialog({ shared, onClose }: { shared: PublicDeckState; onClose: () => void }) {
  const phone = useIsPhone();
  const [decks, setDecks] = useState<Array<{ id: number; name: string }> | null>(null);
  const [mine, setMine] = useState<number | null>(null);
  const [diff, setDiff] = useState<DeckComparison | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { api.listDecks().then((d) => setDecks(d), (e) => setError(message(e))); }, []);
  useEffect(() => {
    if (mine == null) return;
    let current = true;
    setDiff(null); setError(null);
    api.compareShared(mine, shared.deck.slug).then((d) => current && setDiff(d), (e) => current && setError(message(e)));
    return () => { current = false; };
  }, [mine, shared.deck.slug]);
  return (
    <Dialog title={`Compare with ${shared.deck.name}`} width="lg" onClose={onClose} placement={phone ? "sheet" : "center"}
      footer={<Button size={phone ? "touch" : "md"} onClick={onClose}>Close</Button>}>
      <Stack gap={6}>
        <Field label="One of your decks">
          <Select value={mine ?? ""} onChange={(e) => setMine(e.target.value === "" ? null : Number(e.target.value))} data-autofocus="">
            <option value="">Choose a deck…</option>
            {(decks ?? []).map((d) => <option key={d.id} value={d.id}>{d.name}</option>)}
          </Select>
        </Field>
        {decks && decks.length === 0 && <Text tone="muted" as="p">You have no decks yet. Fork this one to start.</Text>}
        {error && <ErrorNote>{error}</ErrorNote>}
        {mine != null && !diff && !error && <Loading />}
        {diff && <DeckDifferences diff={diff} />}
      </Stack>
    </Dialog>
  );
}
