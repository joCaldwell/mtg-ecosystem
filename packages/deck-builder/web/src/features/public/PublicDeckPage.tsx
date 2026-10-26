// `/d/<slug>` (spec §18.5): a shared deck as anyone its visibility admits may
// read it, signed in or not. Everything on the page is PublicDeckState; the
// owner looking at their own link sees the same page with a note saying so.

import { useEffect, useMemo, useState } from "react";
import { api, ApiError } from "../../api/reader.ts";
import type { PublicDeckState } from "../../api/client.ts";
import { APP_TITLE, useScrollRestore } from "../../app/routes.ts";
import { useSession } from "../../app/Session.tsx";
import { fromPublic } from "../../domain/readOnlyDeck.ts";
import { ReadOnlyDeck } from "../deck/index.ts";
import { Banner, Button, EmptyState, Loading } from "../../ui/index.ts";
import { PublicShell } from "./PublicShell.tsx";
import { SharedDeckHeader } from "./SharedDeckHeader.tsx";

type Loaded = { deck: PublicDeckState } | { missing: true } | { error: string };

export function PublicDeckPage({ slug }: { slug: string }) {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let current = true;
    setLoaded(null);
    api.publicDeck(slug).then(
      (r) => current && setLoaded({ deck: r.deck }),
      (e: unknown) => current && setLoaded(e instanceof ApiError && e.status === 404 ? { missing: true } : { error: e instanceof Error ? e.message : String(e) }),
    );
    return () => { current = false; };
  }, [slug, attempt]);

  useEffect(() => {
    if (!loaded || !("deck" in loaded)) return;
    document.title = `${loaded.deck.deck.name} · ${loaded.deck.deck.owner.handle}`;
    // A cold load's head already carries this page's title, so restore the app's own.
    return () => { document.title = APP_TITLE; };
  }, [loaded]);
  useScrollRestore(loaded !== null);

  return (
    <PublicShell>
      {!loaded ? <Loading /> : "missing" in loaded ? (
        <EmptyState title="There is no shared deck here">
          It may be private, or it may have been deleted. Ask whoever sent the link to share it again.
        </EmptyState>
      ) : "error" in loaded ? (
        <EmptyState title="This deck could not be loaded">
          {loaded.error} <Button size="sm" onClick={() => setAttempt((n) => n + 1)}>Retry</Button>
        </EmptyState>
      ) : <SharedDeck shared={loaded.deck} />}
    </PublicShell>
  );
}

function SharedDeck({ shared }: { shared: PublicDeckState }) {
  const { session } = useSession();
  const model = useMemo(() => fromPublic(shared), [shared]);
  // Local mode is one person, who owns every deck; hosted compares handles.
  const own = session.mode === "local" || session.user?.handle === shared.deck.owner.handle;
  const note = own
    ? shared.deck.visibility === "private"
      ? "This is how viewers would see this deck. It is private: only you can open this link. Choose Share in the deck to change that."
      : `This is how viewers see your deck. It is ${shared.deck.visibility}${shared.deck.visibility === "public" ? " and listed on your profile" : ": anyone with the link can read it"}.`
    : null;
  return (
    <ReadOnlyDeck
      model={model}
      header={<>
        {note && <Banner tone="neutral">{note}</Banner>}
        <SharedDeckHeader shared={shared} own={own} />
      </>}
    />
  );
}
