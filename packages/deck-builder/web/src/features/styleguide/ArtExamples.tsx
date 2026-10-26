import styles from "./ArtExamples.module.css";
import {useSession} from "../../app/Session.tsx";
import {api} from "../../api/client.ts";
import {useQuery} from "../../api/useQuery.ts";
import {DeckArt,DeckThumbnail} from "../../mtg/index.ts";
import {Section,Stack,Text} from "../../ui/index.ts";
export function ArtExamples(){
 const {session}=useSession();
 const decks=useQuery(()=>api.listDecks(),[]);
 const art=decks.data?.flatMap(d=>d.commander_art).slice(0,2)??[];
 return <Section title="Deck art · D21"><Stack gap={6}>
  <Text>Decorative Scryfall crops. Credits use the opaque surface; identity uses only the thin stripe. All frames reserve space before images load.</Text>
  <DeckArt cards={art.slice(0,1)} identity="G"><Text>Single crop · green identity</Text></DeckArt>
  <DeckArt cards={art} identity="WUBRG"><Text>Paired crops · split identity · ordinary keyboard focus stays on controls</Text></DeckArt>
  <DeckArt cards={art} variant="banner" identity="UB"><Text>Expanded banner</Text></DeckArt>
  <DeckArt cards={art} variant="banner" collapsed identity="UB"><Text>Folded banner · credits remain</Text></DeckArt>
  <DeckThumbnail cards={art} />
  <DeckArt cards={session.backdrop?[session.backdrop]:[]} variant="backdrop"><Text>Visit backdrop · quiet environment</Text></DeckArt>
  <DeckArt cards={[]}><Text>No commander / loading facts · no art, grey identity</Text></DeckArt>
  <DeckArt cards={art.map(c=>({...c,artist:null}))}><Text>Unknown artist · no art</Text></DeckArt>
  <div className={styles.unavailable}><DeckArt cards={art}><Text>Unavailable image · frame and credits retained</Text></DeckArt></div>
  <Text size="xs">A failed download retains its space and artist credit. No animation; global reduced motion still applies. Thumbnail credit is accessible and has a title.</Text>
 </Stack></Section>;
}
