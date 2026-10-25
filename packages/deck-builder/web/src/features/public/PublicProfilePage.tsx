// `/u/<handle>` (spec §18.5): a person's display name and their public decks,
// nothing else. Unlisted decks are reachable only by their own links.

import { Fragment, useEffect, useState } from "react";
import { api, ApiError, type PublicProfile } from "../../api/client.ts";
import { href, navigate } from "../../app/routes.ts";
import { DECK_CATEGORY_LABELS } from "../../domain/deckCategory.ts";
import { ColorIdentity } from "../../mtg/index.ts";
import { Chip, ClickableRow, EmptyState, Loading, Table, Text } from "../../ui/index.ts";
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
    if (profile && profile !== "missing") document.title = `${profile.display_name} (${profile.handle})`;
  }, [profile]);

  return (
    <PublicShell>
      {error ? <EmptyState title="This profile could not be loaded">{error}</EmptyState>
        : profile === null ? <Loading />
        : profile === "missing" ? <EmptyState title="There is no one with that handle">Check the link, or the person may have deleted their account.</EmptyState>
        : (
          <div className={styles.page}>
            <header className={styles.head}>
              <h1 className={styles.title}>{profile.display_name}</h1>
              <Text tone="muted">@{profile.handle} · {profile.decks.length} public deck{profile.decks.length === 1 ? "" : "s"}</Text>
            </header>
            {profile.decks.length === 0 ? (
              <EmptyState title="No public decks yet">Decks appear here when their owner makes them public.</EmptyState>
            ) : (
              <div className={styles.tableWrap}>
                <Table>
                  <thead><tr><th>Deck</th><th>Commander</th><th>Identity</th><th>Lifecycle</th><th>Cards</th></tr></thead>
                  <tbody>
                    {profile.decks.map((d) => (
                      <ClickableRow key={d.slug} onClick={() => navigate({ page: "shared", slug: d.slug })}>
                        <td>
                          <a href={href({ page: "shared", slug: d.slug })} className={styles.name}>{d.name}</a>
                          {d.parent && <Text tone="faint" size="sm"> forked from {d.parent.owner_handle}/{d.parent.name}</Text>}
                        </td>
                        <td>{d.commanders.length ? d.commanders.map((c, i) => <Fragment key={c}>{i > 0 && " + "}{c}</Fragment>) : <Text tone="faint">none yet</Text>}</td>
                        <td><ColorIdentity colors={d.color_identity} /></td>
                        <td><Chip>{DECK_CATEGORY_LABELS[d.category]}</Chip></td>
                        <td><Text mono tone={d.card_count === 100 ? "muted" : "warn"}>{d.card_count}/100</Text></td>
                      </ClickableRow>
                    ))}
                  </tbody>
                </Table>
              </div>
            )}
          </div>
        )}
    </PublicShell>
  );
}
