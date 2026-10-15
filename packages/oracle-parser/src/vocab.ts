// Closed word classes the parser needs to classify tokens.
// Subtypes are enumerated by the Comprehensive Rules; arbitrary capitalized
// words are not subtypes. Counter names remain an open class.

export const CARD_TYPES = new Set([
  "artifact", "battle", "creature", "enchantment", "instant", "kindred",
  "tribal", "land", "planeswalker", "sorcery",
]);

/** Object classes that head a filter like a type does. */
// A damage source is any qualifying object (CR 120.1), not necessarily a
// permanent: sources include spells and can include abilities on the stack.
export const OBJECT_CLASSES = new Set(["permanent", "spell", "card", "token", "ability", "source"]);

export const SUPERTYPES = new Set(["legendary", "basic", "snow", "world"]);

export const COLORS = new Set(["white", "blue", "black", "red", "green"]);

export const ZONES = new Set([
  "battlefield", "graveyard", "library", "hand", "exile", "stack",
]);

export const STATUS_WORDS = new Set([
  "tapped", "untapped", "attacking", "blocking", "blocked", "enchanted",
  "equipped", "monstrous", "face-up", "face-down", "unblocked",
  // CR 700.9, 701.15b, 701.60b: defined designations/qualifications,
  // distinct from granted keyword abilities.
  // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
  "modified", "goaded", "suspected",
  // CR 712: a double-faced permanent with its back face up.
  "transformed",
  // CR 712.1: a card (or permanent) with a face on each side.
  "double-faced",
]);

const WORD_NUMBERS: Record<string, number> = {
  one: 1, two: 2, three: 3, four: 4, five: 5, six: 6, seven: 7, eight: 8,
  nine: 9, ten: 10, eleven: 11, twelve: 12, thirteen: 13, fourteen: 14,
  fifteen: 15, sixteen: 16, seventeen: 17, eighteen: 18, nineteen: 19,
  twenty: 20, fifty: 50,
};

const TENS: Record<string, number> = { twenty: 20, thirty: 30, forty: 40, fifty: 50, sixty: 60, seventy: 70, eighty: 80, ninety: 90 };

/** "twenty-five": a tens word and a units word joined by a hyphen. */
export function wordNumber(word: string): number | undefined {
  const direct = WORD_NUMBERS[word];
  if (direct !== undefined) return direct;
  const [tens, units, extra] = word.split("-");
  const unit = units === undefined ? undefined : WORD_NUMBERS[units];
  return extra === undefined && TENS[tens] !== undefined && unit !== undefined && unit < 10 ? TENS[tens] + unit : undefined;
}

// CR 205.3g–m,p,q enumerates the nonplanar subtypes. Capitalization alone
// never establishes a subtype: "They", a shortened card name, and "Modified"
// are not creature types. Keep canonical spelling (notably Assembly-Worker),
// and look up exact singular forms before considering English plural suffixes.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const SUBTYPE_NAMES = [
  "Attraction|Blood|Bobblehead|Book|Clue|Contraption|Equipment|Food|Fortification|Gold|Heartwood|Incubator",
  "Infinity|Junk|Lander|Map|Mutagen|Powerstone|Spacecraft|Stone|Treasure|Vehicle|Vibranium|Aura|Background",
  "Cartouche|Case|Class|Curse|Plan|Role|Room|Rune|Saga|Shard|Shrine|Cave|Desert|Forest|Gate|Island|Lair",
  "Locus|Mine|Mountain|Plains|Planet|Power-Plant|Sphere|Swamp|Tower|Town|Urza's|Ajani|Aminatou|Angrath",
  "Arlinn|Ashiok|Bahamut|Basri|Bolas|Calix|Chandra|Comet|Dack|Dakkon|Daretti|Davriel|Dellian|Dihada|Domri",
  "Dovin|Ellywick|Elminster|Elspeth|Estrid|Freyalise|Garruk|Gideon|Grist|Guff|Huatli|Jace|Jared|Jaya|Jeska",
  "Kaito|Karn|Kasmina|Kaya|Kiora|Koth|Liliana|Lolth|Lukka|Minsc|Mordenkainen|Nahiri|Narset|Niko|Nissa",
  "Nixilis|Oko|Quintorius|Ral|Rowan|Saheeli|Samut|Sarkhan|Serra|Sivitri|Sorin|Szat|Tamiyo|Tasha|Teferi|Teyo",
  "Tezzeret|Tibalt|Tyvar|Ugin|Urza|Venser|Vivien|Vraska|Vronos|Will|Windgrace|Wrenn|Xenagos|Yanggu|Yanling",
  "Zariel|Adventure|Arcane|Lesson|Omen|Trap|Time Lord|Advisor|Aetherborn|Alien|Ally|Angel|Antelope|Ape",
  "Archer|Archon|Armadillo|Army|Artificer|Assassin|Assembly-Worker|Astartes|Atog|Aurochs|Avatar|Azra|Badger",
  "Balloon|Barbarian|Bard|Basilisk|Bat|Bear|Beast|Beaver|Beeble|Beholder|Berserker|Bird|Bison|Blinkmoth|Boar",
  "Bringer|Brushwagg|Camarid|Camel|Capybara|Caribou|Carrier|Cat|Centaur|Child|Chimera|Citizen|Cleric|Clown",
  "Cockatrice|Construct|Coward|Coyote|Crab|Crocodile|C'tan|Custodes|Cyberman|Cyclops|Dalek|Dauthi|Demigod",
  "Demon|Deserter|Detective|Devil|Dinosaur|Djinn|Doctor|Dog|Dragon|Drake|Dreadnought|Drix|Drone|Druid|Dryad",
  "Dwarf|Echidna|Efreet|Egg|Elder|Eldrazi|Elemental|Elephant|Elf|Elk|Employee|Eternal|Eye|Faerie|Ferret|Fish",
  "Flagbearer|Fox|Fractal|Frog|Fungus|Gamer|Gamma|Gargoyle|Germ|Giant|Giraffe|Gith|Glimmer|Gnoll|Gnome|Goat",
  "Goblin|God|Golem|Gorgon|Graveborn|Gremlin|Griffin|Guest|Hag|Halfling|Hamster|Harpy|Hedgehog|Hellion|Hero",
  "Hippo|Hippogriff|Homarid|Homunculus|Horror|Horse|Human|Hydra|Hyena|Illusion|Imp|Incarnation|Inhuman",
  "Inkling|Inquisitor|Insect|Jackal|Jellyfish|Juggernaut|Kangaroo|Kavu|Kirin|Kithkin|Knight|Kobold|Kor",
  "Kraken|Kree|Llama|Lamia|Lammasu|Leech|Lemur|Leviathan|Lhurgoyf|Licid|Lizard|Lobster|Manticore|Masticore",
  "Mercenary|Merfolk|Metathran|Minion|Minotaur|Mite|Mole|Monger|Mongoose|Monk|Monkey|Moogle|Moonfolk|Mount",
  "Mouse|Mutant|Myr|Mystic|Nautilus|Necron|Nephilim|Nightmare|Nightstalker|Ninja|Noble|Noggle|Nomad|Nymph",
  "Octopus|Ogre|Ooze|Orb|Orc|Orgg|Otter|Ouphe|Ox|Oyster|Pangolin|Peasant|Pegasus|Pentavite|Performer|Pest",
  "Phelddagrif|Phoenix|Phyrexian|Pilot|Pincher|Pirate|Plant|Platypus|Porcupine|Possum|Praetor|Primarch|Prism",
  "Processor|Qu|Rabbit|Raccoon|Ranger|Rat|Rebel|Reflection|Rhino|Rigger|Robot|Rogue|Sable|Salamander|Samurai",
  "Sand|Saproling|Satyr|Scarecrow|Scientist|Scion|Scorpion|Scout|Sculpture|Seal|Serf|Serpent|Servo|Shade",
  "Shaman|Shapeshifter|Shark|Sheep|Shi'ar|Siren|Skeleton|Skrull|Skunk|Slith|Sliver|Sloth|Slug|Snail|Snake",
  "Soldier|Soltari|Sorcerer|Spawn|Specter|Spellshaper|Sphinx|Spider|Spike|Spirit|Splinter|Sponge|Spy|Squid",
  "Squirrel|Starfish|Surrakar|Survivor|Symbiote|Synth|Tentacle|Tetravite|Thalakos|Thopter|Thrull|Tiefling",
  "Toy|Treefolk|Trilobite|Triskelavite|Troll|Turtle|Tyranid|Unicorn|Utrom|Vampire|Varmint|Vedalken|Villain",
  "Volver|Wall|Walrus|Warlock|Warrior|Weasel|Weird|Werewolf|Whale|Wizard|Wolf|Wolverine|Wombat|Worm|Wraith",
  "Wurm|Yeti|Zombie|Zubera|Undercity|Siege",
].join("|").split("|");

const SUBTYPES = new Map(SUBTYPE_NAMES.map(name => [name.toLowerCase(), name]));
const IRREGULAR_SUBTYPE_PLURALS: Readonly<Record<string, string>> = {
  elves: "Elf", dwarves: "Dwarf", wolves: "Wolf", werewolves: "Werewolf",
  oxen: "Ox", mice: "Mouse", fungi: "Fungus", homunculi: "Homunculus",
  pegasi: "Pegasus", octopi: "Octopus", cyclopes: "Cyclops", loci: "Locus",
};

/** An official subtype, or its plural, in canonical Oracle spelling. */
export function canonicalSubtypeName(raw: string): string | undefined {
  const word = raw.replace(/’/g, "'").toLowerCase();
  const exact = SUBTYPES.get(word);
  if (exact) return exact;
  const irregular = IRREGULAR_SUBTYPE_PLURALS[word];
  if (irregular) return irregular;
  // Require a real singular subtype after removing a plural ending. This
  // resolves both "Heroes" → Hero and "Faeries" → Faerie without damaging
  // invariant singulars such as Plains, Cyclops, Aurochs, Locus, and Astartes.
  if (word.endsWith("s")) {
    const regular = SUBTYPES.get(word.slice(0, -1));
    if (regular) return regular;
  }
  if (word.endsWith("es")) {
    const regular = SUBTYPES.get(word.slice(0, -2));
    if (regular) return regular;
  }
  if (word.endsWith("ies")) return SUBTYPES.get(word.slice(0, -3) + "y");
  return undefined;
}

/**
 * Singular form of a pluralizable rules word. Only used for closed-class and
 * head-noun positions, so a simple suffix rule plus exceptions is enough.
 */
export function singularize(word: string): string {
  const subtype = canonicalSubtypeName(word);
  if (subtype) return subtype.toLowerCase();
  const exceptions: Record<string, string> = {
    sorceries: "sorcery",
    libraries: "library",
    abilities: "ability",
    copies: "copy",
    elves: "elf",
    dwarves: "dwarf",
    wolves: "wolf",
    thopters: "thopter",
    mercenaries: "mercenary",
    allies: "ally",
    armies: "army",
    harpies: "harpy",
    zombies: "zombie",
    faeries: "faerie",
  };
  if (exceptions[word]) return exceptions[word];
  if (word.endsWith("ies") && word.length > 4) return word.slice(0, -3) + "y";
  if (word.endsWith("sses")) return word.slice(0, -2);
  if (word.endsWith("s") && !word.endsWith("ss")) return word.slice(0, -1);
  return word;
}
