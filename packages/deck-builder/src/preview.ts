// Link previews for shared pages (spec §18.5, plan D16/D20). Discord, Slack
// and search engines read the HTML they are served, not what the app renders
// afterwards, so /d/<slug> and /u/<handle> get their tags in index.html.
// Tags follow the Open Graph protocol (https://ogp.me/: og:title, og:type,
// og:image and og:url are the required four) and X's card markup
// (https://developer.x.com/en/docs/x-for-websites/cards/overview/markup).

import type { Database } from "./sqlite.ts";
import type { Actor } from "./actor.ts";
import { ServiceError } from "./errors.ts";
import { publicProfile } from "./sharing.ts";
import { publicDeckState } from "./state.ts";

const SITE = "MTG Deck Builder";
const COLOURS: Record<string, string> = { W: "White", U: "Blue", B: "Black", R: "Red", G: "Green" };

export interface LinkPreview {
  /** 404 when the requester cannot read what the path names. */
  status: 200 | 404;
  head: string;
}

function escape(text: string): string {
  return text.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

function identityName(identity: string): string {
  return identity ? identity.split("").map((c) => COLOURS[c] ?? c).join(", ") : "Colourless";
}

function tags(t: { title: string; description: string; url: string; image?: string | null; type: string; index: boolean }): string {
  const meta = (attr: "name" | "property", key: string, value: string) => `<meta ${attr}="${key}" content="${escape(value)}" />`;
  return [
    `<title>${escape(t.title)}</title>`,
    meta("name", "description", t.description),
    ...(t.index ? [] : [meta("name", "robots", "noindex")]),
    meta("property", "og:site_name", SITE),
    meta("property", "og:type", t.type),
    meta("property", "og:title", t.title),
    meta("property", "og:description", t.description),
    meta("property", "og:url", t.url),
    ...(t.image ? [meta("property", "og:image", t.image)] : []),
    meta("name", "twitter:card", t.image ? "summary_large_image" : "summary"),
  ].join("\n    ");
}

/** The head tags for a shared page, or null when the path is not one. */
export function linkPreview(db: Database, viewer: Actor | null, pathname: string, origin: string): LinkPreview | null {
  const deck = pathname.match(/^\/d\/([^/]+)$/);
  const user = pathname.match(/^\/u\/([^/]+)$/);
  if (!deck && !user) return null;
  const url = `${origin}${pathname}`;
  const missing = (what: string): LinkPreview => ({
    status: 404,
    head: tags({ title: `${what} not found · ${SITE}`, description: "This page is private or no longer exists.", url, type: "website", index: false }),
  });
  try {
    if (deck) {
      const shared = publicDeckState(db, viewer, decodeURIComponent(deck[1]));
      const commanders = shared.cards.filter((c) => c.role === "commander" && !c.maybe);
      const target = shared.power.target?.mapping;
      const bracket = target?.bracket != null ? `Bracket ${target.bracket}${target.provisional ? " (provisional)" : ""}${target.name ? ` · ${target.name}` : ""} target` : null;
      const art = commanders.find((c) => c.image_art_crop)?.image_art_crop ?? null;
      const artist = commanders.find((c) => c.image_art_crop)?.artist ?? null;
      const description = [
        commanders.length ? commanders.map((c) => c.name).join(" and ") : "No commander yet",
        identityName(shared.deck.color_identity),
        bracket,
        `Commander deck by ${shared.deck.owner.handle}`,
        artist && art ? `Art by ${artist}` : null,
      ].filter(Boolean).join(" · ");
      return {
        status: 200,
        head: tags({ title: `${shared.deck.name} · ${shared.deck.owner.handle}`, description, url, image: art, type: "website", index: shared.deck.visibility === "public" }),
      };
    }
    const profile = publicProfile(db, viewer, decodeURIComponent(user![1]));
    const count = profile.decks.length;
    return {
      status: 200,
      head: tags({
        title: `${profile.display_name} (${profile.handle}) · ${SITE}`,
        description: `${count} public Commander deck${count === 1 ? "" : "s"} by ${profile.handle}.`,
        url, type: "profile", index: true,
      }),
    };
  } catch (e) {
    if (e instanceof ServiceError && e.status === 404) return missing(deck ? "Deck" : "Profile");
    if (e instanceof URIError) return missing(deck ? "Deck" : "Profile");
    throw e;
  }
}

/** index.html with the preview's tags in place of its own title. */
export function withPreview(html: string, preview: LinkPreview): string {
  const head = html.replace(/<title>[^<]*<\/title>/, "");
  return head.replace("</head>", `  ${preview.head}\n  </head>`);
}
