// A shared deck's header (spec §18.5, D20): the commander's art crop with its
// artist line, the deck's name and owner, and what a reader can do with it.
// The art is the stored Scryfall CDN URL (D10); without a stored artist the
// picture is left out rather than shown uncredited.

import { Fragment, useState, lazy, Suspense } from "react";
import type { PublicDeckState } from "../../api/client.ts";
import { href, signInHref } from "../../app/routes.ts";
import { useIsPhone } from "../../app/breakpoints.ts";
import { useSession } from "../../app/Session.tsx";
import { bracketLabel } from "../../domain/power.ts";
import { DECK_CATEGORY_LABELS } from "../../domain/deckCategory.ts";
import { CardRef, ColorIdentity, DeckArt } from "../../mtg/index.ts";
import { Button, Chip, Icon, useToast } from "../../ui/index.ts";
const CompareSharedDialog = lazy(() => import("./SharedDialogs.tsx").then(m => ({ default: m.CompareSharedDialog })));
const ExportSharedDialog = lazy(() => import("./SharedDialogs.tsx").then(m => ({ default: m.ExportSharedDialog })));
const ForkSharedDialog = lazy(() => import("./SharedDialogs.tsx").then(m => ({ default: m.ForkSharedDialog })));
import styles from "./SharedDeckHeader.module.css";

type Open = "fork" | "compare" | "export" | null;

export function SharedDeckHeader({ shared, own }: { shared: PublicDeckState; own: boolean }) {
  const { session } = useSession();
  const { toast } = useToast();
  const [open, setOpen] = useState<Open>(null);
  const { deck, power } = shared;
  const signedIn = session.mode === "local" || !!session.user;
  const phone = useIsPhone();
  const commanders = shared.cards.filter((c) => c.role === "commander" && !c.maybe);
  const link = `${location.origin}${href({ page: "shared", slug: deck.slug })}`;

  async function copyLink() {
    try {
      await navigator.clipboard.writeText(link);
      toast("Link copied.");
    } catch {
      toast(`Copy this link: ${link}`);
    }
  }

  return (
    <DeckArt cards={commanders} identity={deck.color_identity} variant="banner">
      <div className={styles.text}>
        <div className={styles.meta}>
          <Chip>{DECK_CATEGORY_LABELS[deck.category]}</Chip>
          {deck.visibility !== "private" && (
            <span className={styles.visibility}>
              <Icon name={deck.visibility === "public" ? "globe" : "link"} />
              {deck.visibility === "public" ? "Public" : "Unlisted"}
            </span>
          )}
          <ColorIdentity colors={deck.color_identity} />
        </div>
        <h1 className={styles.title}>{deck.name}</h1>
        <p className={styles.byline}>
          {/* A profile exists only while its owner has a public deck (§18.5); an
              unlisted deck cannot know that, so it names the owner without a link. */}
          by {deck.visibility === "public"
            ? <a href={href({ page: "user", handle: deck.owner.handle })}>{deck.owner.display_name}</a>
            : deck.owner.display_name}{" "}
          <span className={styles.handle}>@{deck.owner.handle}</span>
          {deck.parent && (
            <>
              {" · forked from "}
              {deck.parent.slug
                ? <a href={href({ page: "shared", slug: deck.parent.slug })}>{deck.parent.owner_handle}/{deck.parent.name}</a>
                : <>{deck.parent.owner_handle}/{deck.parent.name}</>}
            </>
          )}
        </p>
        <p className={styles.commanders}>
          {commanders.length
            ? commanders.map((c, i) => <Fragment key={c.oracle_id}>{i > 0 && " + "}<CardRef name={c.name} /></Fragment>)
            : "No commander yet"}
          {power.target && <span className={styles.bracket}> · target {bracketLabel(power.target.mapping)}{power.target.mapping.name ? ` ${power.target.mapping.name}` : ""}</span>}
        </p>
        <div className={styles.actions}>
          <Button size="touch" onClick={() => void copyLink()}><Icon name="link" /> Copy link</Button>
          <Button size="touch" onClick={() => setOpen("export")}>Export</Button>
          {signedIn ? (
            <>
              {/* A phone forks other people's decks (§11.2); its owner forks their own on a larger screen. */}
              {!(own && phone) && <Button size="touch" variant="primary" onClick={() => setOpen("fork")}>{own ? "Fork your deck" : "Fork"}</Button>}
              <Button size="touch" onClick={() => setOpen("compare")}>Compare with one of my decks</Button>
            </>
          ) : (
            <a className={styles.signin} href={signInHref(href({ page: "shared", slug: deck.slug }))}>Sign in to fork or compare</a>
          )}
        </div>
      </div>
      <Suspense fallback={null}>
      {open === "export" && <ExportSharedDialog shared={shared} onClose={() => setOpen(null)} />}
      {open === "fork" && <ForkSharedDialog shared={shared} onClose={() => setOpen(null)} />}
      {open === "compare" && <CompareSharedDialog shared={shared} onClose={() => setOpen(null)} />}
      </Suspense>
    </DeckArt>
  );
}
