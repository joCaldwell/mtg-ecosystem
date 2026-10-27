import styles from "./ArtExamples.module.css";
import { useSession } from "../../app/Session.tsx";
import type { ArtFacts } from "../../domain/art.ts";
import { DeckArt, DeckThumbnail } from "../../mtg/index.ts";
import { Section, Stack, Text } from "../../ui/index.ts";

// Fixed examples, so the reference looks the same for every viewer. These are
// stored bulk facts copied from the cards table (checked 2026-10-26), not
// typed from memory; the backdrop is the session's own visit choice.
const AKIRI: ArtFacts = { oracle_id: "3e0a0276-b640-41c7-95bd-ba3067f5c911", name: "Akiri, Line-Slinger", artist: "David Gaillet",
  image_art_crop: "https://cards.scryfall.io/art_crop/front/3/b/3b951e0c-a4dd-4a20-87c6-eaa947e33aa4.jpg?1783937085" };
const ALENA: ArtFacts = { oracle_id: "ca349ba9-b5ea-48e6-a617-c9b7f2708853", name: "Alena, Kessig Trapper", artist: "Zoltan Boros",
  image_art_crop: "https://cards.scryfall.io/art_crop/front/9/0/90c51949-b1ef-4c6e-98b0-4fb33a81db24.jpg?1783928824" };
const RUNE_TAIL: ArtFacts = { oracle_id: "710f1f8a-e5f7-4889-9f12-2611f42d5c73", name: "Rune-Tail, Kitsune Ascendant // Rune-Tail's Essence", artist: "Randy Gallegos",
  image_art_crop: "https://cards.scryfall.io/art_crop/front/4/2/42ba0e13-d20f-47f9-9c86-2b0b13c39ada.jpg?1783944168" };
const PAIR = [AKIRI, ALENA];

function Example({ label, children }: { label: string; children: string }) {
  return <div className={styles.body}><Text weight="medium">{label}</Text><Text as="p" tone="muted" size="sm">{children}</Text></div>;
}

export function ArtExamples() {
  const { session } = useSession();
  return <Section title="Deck art · D21"><Stack gap={6}>
    <Text>Decorative Scryfall crops in a band above the content, each credited inside its own crop. Identity is only the thin stripe. Frames reserve their space before images load.</Text>
    <div className={styles.grid}>
      <DeckArt cards={[AKIRI]} identity="W"><Example label="Strip · single">Roster card. The commander is named on the card, so the credit is the artist alone.</Example></DeckArt>
      <DeckArt cards={PAIR} identity="WR"><Example label="Strip · partners">Two crops, one credit in each half.</Example></DeckArt>
      <DeckArt cards={[RUNE_TAIL]} identity="W"><Example label="Strip · double-faced">Credited by its front face.</Example></DeckArt>
      <DeckArt cards={[]}><Example label="No commander">No art; the grey stripe stands in for identity.</Example></DeckArt>
      <DeckArt cards={[{ ...AKIRI, artist: null }]} identity="W"><Example label="Unknown artist">Uncredited art is never drawn.</Example></DeckArt>
      <div className={styles.unavailable}><DeckArt cards={PAIR} identity="WR"><Example label="Image failed">The frame keeps its space; the credit goes with the picture.</Example></DeckArt></div>
    </div>
    <DeckArt cards={PAIR} variant="banner" identity="WR"><Example label="Banner">Deck header. Folds on desktop while the list is scrolled.</Example></DeckArt>
    <DeckArt cards={PAIR} variant="banner" collapsed identity="WR"><Example label="Banner · folded">Art and credit fade out with the band.</Example></DeckArt>
    <DeckArt cards={[AKIRI]} variant="showcase" identity="W"><Example label="Showcase">Public deck page, loaded first.</Example></DeckArt>
    <DeckArt cards={session.backdrop ? [session.backdrop] : [ALENA]} variant="backdrop"><Example label="Backdrop">Sign-in and first run. No card is named nearby, so the credit names it.</Example></DeckArt>
    <div className={styles.thumbs}>
      <DeckThumbnail cards={[AKIRI]} className={styles.dim} /><Text size="sm" tone="muted">Rail thumbnail, dimmed</Text>
      <DeckThumbnail cards={PAIR} /><Text size="sm" tone="muted">Current or hovered; partners split it. Decorative, credit in the tooltip.</Text>
    </div>
    <Text size="xs" tone="muted">Banners, showcases and backdrops load first; roster and rail art loads as it nears view. The fold animates with --base and is instant under reduced motion.</Text>
  </Stack></Section>;
}
