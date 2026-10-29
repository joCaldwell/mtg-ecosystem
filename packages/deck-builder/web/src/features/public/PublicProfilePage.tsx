// `/u/<handle>` (spec §18.5): a person's display name and their public decks,
// nothing else. Unlisted decks are reachable only by their own links.

import { useEffect, useState } from "react";
import { api, ApiError } from "../../api/reader.ts";
import type { PublicProfile } from "../../api/client.ts";
import { APP_TITLE, href, useScrollRestore } from "../../app/routes.ts";
import { DeckCard } from "../../mtg/index.ts";
import { EmptyState, Loading, PageHeader, Text } from "../../ui/index.ts";
import { PublicShell } from "./PublicShell.tsx";
import styles from "./PublicProfilePage.module.css";

export function PublicProfilePage({ handle }: { handle: string }) {
  const [profile, setProfile] = useState<PublicProfile | null | "missing">(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let current = true;
    setProfile(null);
    api.publicProfile(handle).then(
      (r) => current && setProfile(r.profile),
      (e: unknown) => current && (e instanceof ApiError && e.status === 404 ? setProfile("missing") : setError(e instanceof Error ? e.message : String(e))),
    );
    return () => { current = false; };
  }, [handle]);
  useEffect(() => {
    if (!profile || profile === "missing") return;
    document.title = `${profile.display_name} (${profile.handle})`;
    // A cold load's head already carries this page's title, so restore the app's own.
    return () => { document.title = APP_TITLE; };
  }, [profile]);
  useScrollRestore(profile !== null);

  return (
    <PublicShell>
      {error ? <EmptyState heading="h1" title="This profile could not be loaded">{error}</EmptyState>
        : profile === null ? <Loading />
        : profile === "missing" ? <EmptyState heading="h1" title="There is no one with that handle">Check the link, or the person may have deleted their account.</EmptyState>
        : (
          <div className={styles.page}>
            <PageHeader title={profile.display_name}
              meta={<>@{profile.handle} · <span className={styles.count}>{profile.decks.length}</span> public deck{profile.decks.length === 1 ? "" : "s"}</>} />
            <div className={styles.content}>
            {profile.decks.length === 0 ? (
              <EmptyState title="No public decks yet">Decks appear here when their owner makes them public.</EmptyState>
            ) : (
              <ul className={styles.roster}>
                {profile.decks.map(d => <DeckCard key={d.slug} name={d.name} href={href({ page: "shared", slug: d.slug })}
                  art={d.commander_art} identity={d.color_identity} category={d.category} count={d.card_count}
                  commanders={d.commanders.join(" + ") || <Text tone="faint">none yet</Text>}
                  lineage={d.parent && <>Fork of {d.parent.slug ? <a href={href({ page: "shared", slug: d.parent.slug })}>{d.parent.owner_handle}/{d.parent.name}</a> : `${d.parent.owner_handle}/${d.parent.name}`}</>} />)}
              </ul>
            )}
            </div>
          </div>
        )}
    </PublicShell>
  );
}
