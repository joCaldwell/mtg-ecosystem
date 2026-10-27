import { useWindowScroll } from "../../app/windowScroll.ts";
import { DeckArt } from "../../mtg/index.ts";
import { useState } from "react";
import { api } from "../../api/client.ts";
import { safeNext } from "../../app/routes.ts";
import { useSession } from "../../app/Session.tsx";
import { Button, ErrorNote, Field, PageHeader, Stack, Text, TextField, Row } from "../../ui/index.ts";
import { LegalFooter } from "./LegalFooter.tsx";
import styles from "./SignInPage.module.css";

export function SignInPage() {
  useWindowScroll();
  const {session} = useSession();
  const [code,setCode] = useState(""), [busy,setBusy] = useState(false), [error,setError] = useState<string | null>(null);
  async function signIn(provider: string) {
    setBusy(true); setError(null);
    try {
      if (code.trim() || !session.invites_required) await api.signup(code.trim());
      // A return path rides beside, never inside, Better Auth's signed oauth_query.
      const query = new URLSearchParams(location.search);
      const next = safeNext(query.get("next"));
      query.delete("next");
      const result = await api.signIn(provider, query.toString(), next);
      location.assign(result.url);
    } catch(e) { setError(e instanceof Error ? e.message : String(e)); setBusy(false); }
  }
  const providerError = new URLSearchParams(location.search).get("error");
  return <main className={styles.page} id="main-content" tabIndex={-1} aria-label="Sign in"><div className={styles.content}><PageHeader title="Your decks, your decisions" />
    <DeckArt cards={session.backdrop ? [session.backdrop] : null} variant="backdrop" priority="auto"><div className={styles.form}><Stack gap={5}>
      <Text>Keep your decks and deck-building knowledge together. Connect an agent when you want help; every write stays attributed to your account.</Text>
      <Field label="Invite or owner claim code" hint={session.invites_required ? "Required for a new account. Leave blank if you have signed in before." : "Optional: enter an owner claim code or invite, or leave blank to create an account."}>
        <TextField type="password" autoComplete="off" value={code} maxLength={256} onChange={e => setCode(e.target.value)} />
      </Field>
      <Row gap={3} wrap>{session.providers.map(p => <Button key={p} variant="primary" disabled={busy} onClick={() => signIn(p)}>Continue with {p === "discord" ? "Discord" : "Google"}</Button>)}</Row>
      {error && <ErrorNote>{error}</ErrorNote>}
      {providerError && <ErrorNote>{providerError === "registration_failed" ? "Registration did not complete. Check your invite or claim code and try again." : "The provider sign-in did not complete or its state expired. Start sign-in again."}</ErrorNote>}
      <Text size="sm" tone="muted" className={styles.legal}>By continuing, you agree to the <a href="/terms">terms</a> and acknowledge the <a href="/privacy">privacy policy</a>.</Text>
    </Stack></div></DeckArt><LegalFooter />
  </div></main>;
}
