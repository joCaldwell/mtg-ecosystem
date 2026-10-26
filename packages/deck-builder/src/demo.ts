import { ServiceError } from "./errors.ts";
// The sample is isolated to one new deck in the requesting user's account.
// Cards resolve against installed data; a missing commander fails atomically.
import type { Database } from "./sqlite.ts";
import { requireUi, type Actor } from "./actor.ts";
import { getSetting, setSetting, withTransaction } from "./db.ts";
import type { Change } from "./deck/membership.ts";
import type { Role } from "./deck/service.ts";
import type { ProposalItemInput } from "./deck/proposals.ts";
import { tryResolveCard } from "./cards.ts";
import { setEngine, updateBrief } from "./deck/brief.ts";
import { applyChanges } from "./deck/membership.ts";
import { acceptItems, createProposal, pendingItemIds, rejectItems } from "./deck/proposals.ts";
import {
  createDeck,
  createSlot,
  createTag,
  organizeCard,
  setDeckCategory,
  shortlistCard,
} from "./deck/service.ts";
import { proposeDocumentEdit } from "./documents.ts";
import { recordActivity } from "./activity.ts";
import { remember } from "./memory.ts";

export function createDemo(db: Database, actor: Actor): { deckId: number; missing: string[] } {
  requireUi(actor, "create a sample deck");
  return withTransaction(db, () => {
  const OWNER = actor;
  const AGENT: Actor = { user_id: actor.user_id, via: "agent", label: "sample-agent" };
  const DECK_NAME = "Sample — Atraxa Superfriends";
  // Repeated requests return the same deck. Deleting it lets the
  // owner create another; no existing deck or user-authored text is replaced.
  const previous = getSetting(db, actor, "sample_deck_id");
  if (previous && db.prepare("SELECT 1 FROM decks WHERE id=? AND user_id=?").get(Number(previous), actor.user_id))
    return { deckId: Number(previous), missing: [] };
// Built as Theorycrafted, classified as Paper at the end: that is the life of a real deck.
const deckId = createDeck(db, OWNER, DECK_NAME, {});

const missing = new Set<string>();
function oracle(name: string) {
  const hit = tryResolveCard(db, name);
  if ("problem" in hit) {
    missing.add(name);
    return null;
  }
  return hit.oracle_id;
}

const slots = {
  manaBase: createSlot(db, OWNER, deckId, "mana base", 36, 38),
  ramp: createSlot(db, OWNER, deckId, "ramp", 10, 12),
  walkers: createSlot(db, OWNER, deckId, "walkers", 8, 12),
  proliferate: createSlot(db, OWNER, deckId, "proliferate", 6, 10),
  interaction: createSlot(db, OWNER, deckId, "interaction", 6, 10),
  draw: createSlot(db, OWNER, deckId, "draw", 5, 8),
  defense: createSlot(db, OWNER, deckId, "defense", 3, 5),
};
const tags = {
  proliferate: createTag(db, OWNER, deckId, "proliferate"),
  removal: createTag(db, OWNER, deckId, "removal"),
  ultimate: createTag(db, OWNER, deckId, "ultimate"),
  wincon: createTag(db, OWNER, deckId, "wincon"),
};

// One logged act for the whole first build, as an import or an agent's
// edit_cards would be.
const build: Change[] = [];
function add(name: string, slot: number | null, opts: { role?: Role; quantity?: number } = {}) {
  const oid = oracle(name);
  if (!oid) return null;
  build.push({ action: "add", oracle_id: oid, slot_id: opts.role ? undefined : (slot ?? undefined), role: opts.role, quantity: opts.quantity });
  return oid;
}

// Exact resolution is mandatory for the sample's commander.
if (!add("Atraxa, Praetors' Voice", null, { role: "commander" }))
  throw new ServiceError("The sample deck needs card data. Ask the administrator to refresh card data in Settings, then try again.");

// Deliberately leaves `walkers` under target and `ramp` over it, so the slot
// deltas and the checks have something real to report.
const groups: [number, string[]][] = [
  [slots.ramp, ["Sol Ring", "Arcane Signet", "Fellwar Stone", "Chromatic Lantern", "Cultivate",
    "Nature's Lore", "Rampant Growth", "Farseek", "Birds of Paradise", "Llanowar Elves",
    "Kodama's Reach", "Three Visits", "Mind Stone"]],
  [slots.walkers, ["Teferi, Temporal Archmage", "Vraska, Golgari Queen", "Karn Liberated",
    "Ajani Steadfast", "Narset Transcendent"]],
  [slots.proliferate, ["Flux Channeler", "Evolution Sage", "Inexorable Tide", "Contagion Engine",
    "Contagion Clasp", "Thrummingbird", "Tekuthal, Inquiry Dominus"]],
  [slots.interaction, ["Swords to Plowshares", "Path to Exile", "Anguished Unmaking",
    "Assassin's Trophy", "Beast Within", "Cyclonic Rift", "Vindicate"]],
  [slots.draw, ["Rhystic Study", "Sylvan Library", "Phyrexian Arena", "Night's Whisper",
    "Painful Truths"]],
  [slots.defense, ["Ghostly Prison", "Propaganda", "Norn's Annex"]],
  [slots.manaBase, ["Command Tower", "Exotic Orchard", "Breeding Pool", "Godless Shrine",
    "Hallowed Fountain", "Overgrown Tomb", "Temple Garden", "Watery Grave", "Yavimaya Coast",
    "Reflecting Pool", "Path of Ancestry", "Bojuka Bog", "Karn's Bastion"]],
];
for (const [slot, names] of groups) for (const n of names) add(n, slot);

// Basics fill the mana base slot toward its target and exercise the quantity path.
add("Forest", slots.manaBase, { quantity: 6 });
add("Plains", slots.manaBase, { quantity: 5 });
add("Island", slots.manaBase, { quantity: 5 });
add("Swamp", slots.manaBase, { quantity: 5 });

applyChanges(db, OWNER, deckId, build, { kind: "edit", summary: "First build" });

for (const [name, tagIds] of [
  ["Flux Channeler", [tags.proliferate]],
  ["Inexorable Tide", [tags.proliferate, tags.wincon]],
  ["Contagion Engine", [tags.proliferate]],
  ["Karn Liberated", [tags.ultimate, tags.wincon]],
  ["Swords to Plowshares", [tags.removal]],
  ["Assassin's Trophy", [tags.removal]],
] as [string, number[]][]) {
  const oid = oracle(name);
  if (oid) organizeCard(db, OWNER, deckId, oid, { tagIds });
}

updateBrief(db, OWNER, deckId, {
  thesis:
    "Atraxa proliferates planeswalker loyalty every end step until an ultimate ends the game. " +
    "The deck is a control shell first: survive, stick a walker, then let the proliferate engine " +
    "outpace what any one opponent can remove.",
  constraints_md:
    "- Casual power level; no infinite combos.\n" +
    "- The playgroup dislikes counterspells — do not suggest them.\n" +
    "- Roughly $20 per card ceiling.\n" +
    "- Keep the curve under 4 outside of the walkers.",
});

const enginePieces = ["Flux Channeler", "Evolution Sage", "Inexorable Tide", "Contagion Engine"]
  .map(oracle)
  .filter((id): id is string => !!id)
  .map((oracle_id) => ({ oracle_id }));
if (enginePieces.length)
  setEngine(db, OWNER, deckId, "proliferate loop", "Cheap proliferate triggers that stack with Atraxa's end step.", enginePieces);


setDeckCategory(db, OWNER, deckId, "paper");

// --- history: the rulings log is the version history, so give it one ---

const itemFor = (name: string, action: "add" | "cut", reason: string, extra: { slot_id?: number; group?: string } = {}) => {
  const oid = oracle(name);
  return oid ? { action, oracle_id: oid, reason, ...extra } : null;
};
function propose(summary: string, items: (ProposalItemInput | null)[]) {
  const real = items.filter((item): item is ProposalItemInput => item !== null);
  return real.length ? createProposal(db, AGENT, deckId, { summary, items: real }) : null;
}

// An accepted add, so the log has an accept in it.
{
  const pid = propose("Loyalty doubling", [
    itemFor("Doubling Season", "add", "Doubles loyalty counters — every walker arrives one activation from its ultimate.", { slot_id: slots.walkers }),
  ]);
  if (pid) acceptItems(db, OWNER, pendingItemIds(db, OWNER, pid), "Yes. This is the deck.");
}

// Two rejections with reasons. The ruling follows the card from now on, and
// an agent turned each into a memory.
{
  const pid = propose("Interaction top-up", [
    itemFor("Counterspell", "add", "Cheapest possible answer to a combo turn.", { slot_id: slots.interaction }),
  ]);
  if (pid) {
    const log = rejectItems(db, OWNER, pendingItemIds(db, OWNER, pid), "The playgroup hates counterspells. Never suggest one.");
    remember(db, AGENT, {
      text: "The owner's playgroup dislikes counterspells; they rejected [[Counterspell]] for that reason and do not want counterspells suggested.",
      scope: "deck", deck_id: deckId,
      basis: "stated",
      source_log_id: log,
    });
    recordActivity(db, AGENT, "remember", deckId, "Memory: the playgroup dislikes counterspells");
  }
}
{
  const pid = propose("Stax angle", [
    itemFor("Static Orb", "add", "Slows the table down while walkers tick up.", { slot_id: slots.defense }),
  ]);
  if (pid) {
    const log = rejectItems(db, OWNER, pendingItemIds(db, OWNER, pid), "Tried it — symmetrical enough that it cost me the games I was ahead in.");
    remember(db, AGENT, {
      text: "[[Static Orb]] was tried here and cut: its symmetry cost games the deck was already winning.",
      scope: "deck",
      deck_id: deckId,
      basis: "stated",
      source_log_id: log,
    });
  }
}

// What an agent concluded on its own: a hypothesis until the owner confirms it.
remember(db, AGENT, {
  text: "Seems to prefer cutting symmetrical effects generally, not only in this deck: every stax piece proposed so far has been rejected for symmetry.",
  scope: "deck", deck_id: deckId,
  basis: "inferred",
});
// The owner's own notes after goldfishing.
remember(db, OWNER, {
  text: "Goldfished ten games: engine online turn 6 on the play, turn 8 on the draw. Never lost to running out of gas — lost to not interacting.",
  scope: "deck",
  deck_id: deckId,
});
remember(db, OWNER, {
  text: "Two games ended with a walker one activation short of an ultimate. More proliferate, not more walkers.",
  scope: "deck",
  deck_id: deckId,
});

// Left pending on purpose: an atomic swap, so the grouped ruling has a case.
propose("Swap a rock for an extra walker activation", [
  itemFor("Mind Stone", "cut", "Weakest rock in an already-over-target ramp slot.", { group: "swap-1" }),
  itemFor("Oath of Teferi", "add", "An extra activation every turn is worth more here than the fourth mana rock.", { slot_id: slots.walkers, group: "swap-1" }),
]);

// A proposed edit to the brief, and one to the profile, for the diff view.
proposeDocumentEdit(
  db,
  AGENT,
  deckId,
  "constraints",
  { content: "- Casual power level; no infinite combos.\n- No counterspells.\n- Roughly $20 per card ceiling.\n- Keep the curve under 4 outside of the walkers.\n- Life total is a resource; symmetrical taxes are not." },
  "Every stax piece suggested so far has been cut for symmetry. The constraint should say so.",
);
// A shortlisted candidate, and a fork an agent is free to rework.
{
  const oid = oracle("The Chain Veil");
  if (oid) shortlistCard(db, OWNER, deckId, oid, "Extra activations if the walker count goes up.");
}
recordActivity(db, AGENT, "create_sample", deckId, "Created sample deck with example proposals and memories");
setSetting(db, actor, "sample_deck_id", String(deckId));
return { deckId, missing: [...missing] };
  });
}
