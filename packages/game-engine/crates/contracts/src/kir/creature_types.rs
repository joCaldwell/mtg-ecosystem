//! The creature subtype vocabulary in pinned CR 205.3m (2026-09-25).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

const SINGLE_WORD_SUBTYPES: &str = "
Advisor Aetherborn Alien Ally Angel Antelope Ape Archer Archon Armadillo Army Artificer
Assassin Assembly-Worker Astartes Atog Aurochs Avatar Azra Badger Balloon Barbarian Bard
Basilisk Bat Bear Beast Beaver Beeble Beholder Berserker Bird Bison Blinkmoth Boar Bringer
Brushwagg Camarid Camel Capybara Caribou Carrier Cat Centaur Child Chimera Citizen Cleric
Clown Cockatrice Construct Coward Coyote Crab Crocodile C’tan Custodes Cyberman Cyclops Dalek
Dauthi Demigod Demon Deserter Detective Devil Dinosaur Djinn Doctor Dog Dragon Drake
Dreadnought Drix Drone Druid Dryad Dwarf Echidna Efreet Egg Elder Eldrazi Elemental Elephant
Elf Elk Employee Eternal Eye Faerie Ferret Fish Flagbearer Fox Fractal Frog Fungus Gamer Gamma
Gargoyle Germ Giant Giraffe Gith Glimmer Gnoll Gnome Goat Goblin God Golem Gorgon Graveborn
Gremlin Griffin Guest Hag Halfling Hamster Harpy Hedgehog Hellion Hero Hippo Hippogriff
Homarid Homunculus Horror Horse Human Hydra Hyena Illusion Imp Incarnation Inhuman Inkling
Inquisitor Insect Jackal Jellyfish Juggernaut Kangaroo Kavu Kirin Kithkin Knight Kobold Kor
Kraken Kree Llama Lamia Lammasu Leech Lemur Leviathan Lhurgoyf Licid Lizard Lobster Manticore
Masticore Mercenary Merfolk Metathran Minion Minotaur Mite Mole Monger Mongoose Monk Monkey
Moogle Moonfolk Mount Mouse Mutant Myr Mystic Nautilus Necron Nephilim Nightmare Nightstalker
Ninja Noble Noggle Nomad Nymph Octopus Ogre Ooze Orb Orc Orgg Otter Ouphe Ox Oyster Pangolin
Peasant Pegasus Pentavite Performer Pest Phelddagrif Phoenix Phyrexian Pilot Pincher Pirate
Plant Platypus Porcupine Possum Praetor Primarch Prism Processor Qu Rabbit Raccoon Ranger Rat
Rebel Reflection Rhino Rigger Robot Rogue Sable Salamander Samurai Sand Saproling Satyr
Scarecrow Scientist Scion Scorpion Scout Sculpture Seal Serf Serpent Servo Shade Shaman
Shapeshifter Shark Sheep Shi’ar Siren Skeleton Skrull Skunk Slith Sliver Sloth Slug Snail
Snake Soldier Soltari Sorcerer Spawn Specter Spellshaper Sphinx Spider Spike Spirit Splinter
Sponge Spy Squid Squirrel Starfish Surrakar Survivor Symbiote Synth Tentacle Tetravite
Thalakos Thopter Thrull Tiefling Toy Treefolk Trilobite Triskelavite Troll Turtle Tyranid
Unicorn Utrom Vampire Varmint Vedalken Villain Volver Wall Walrus Warlock Warrior Weasel Weird
Werewolf Whale Wizard Wolf Wolverine Wombat Worm Wraith Wurm Yeti Zombie Zubera";

/// A validated choice from CR 205.3m, serialized by its printed name.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(try_from = "String", into = "String")]
#[schemars(with = "String")]
pub struct CreatureType(u16);

impl CreatureType {
    pub fn all() -> impl Iterator<Item = Self> {
        Self::names()
            .enumerate()
            .map(|(index, _)| Self(index as u16))
    }

    fn names() -> impl Iterator<Item = &'static str> {
        SINGLE_WORD_SUBTYPES
            .split_ascii_whitespace()
            .chain(["Time Lord"])
    }

    pub fn index(self) -> u16 {
        self.0
    }

    pub fn name(self) -> &'static str {
        Self::names()
            .nth(usize::from(self.0))
            .expect("validated creature type")
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::names()
            .position(|kind| kind == name)
            .map(|index| Self(index as u16))
    }
}

impl TryFrom<String> for CreatureType {
    type Error = &'static str;
    fn try_from(name: String) -> Result<Self, Self::Error> {
        Self::from_name(&name).ok_or("not a creature type")
    }
}
impl From<CreatureType> for String {
    fn from(kind: CreatureType) -> Self {
        kind.name().into()
    }
}

pub fn is_creature_subtype(subtype: &str) -> bool {
    CreatureType::from_name(subtype).is_some()
}

/// CR 205.3i land types; the first five are the basic land types (CR 305.6).
const LAND_SUBTYPES: &[&str] = &[
    "Plains",
    "Island",
    "Swamp",
    "Mountain",
    "Forest",
    "Cave",
    "Desert",
    "Gate",
    "Lair",
    "Locus",
    "Mine",
    "Planet",
    "Power-Plant",
    "Sphere",
    "Tower",
    "Town",
    "Urza’s",
];
/// CR 205.3g artifact types.
const ARTIFACT_SUBTYPES: &str = "Attraction Blood Bobblehead Book Clue Contraption Equipment Food
Fortification Gold Heartwood Incubator Infinity Junk Lander Map Mutagen Powerstone Spacecraft
Stone Treasure Vehicle Vibranium";
/// CR 205.3h enchantment types.
const ENCHANTMENT_SUBTYPES: &str =
    "Aura Background Cartouche Case Class Curse Plan Role Room Rune Saga Shard Shrine";
/// CR 205.3k spell types.
const SPELL_SUBTYPES: &str = "Adventure Arcane Lesson Omen Trap";

/// CR 205.3i: a land type.
pub fn is_land_subtype(subtype: &str) -> bool {
    LAND_SUBTYPES.contains(&subtype) || subtype == "Urza's"
}

/// CR 305.6: one of the five basic land types.
pub fn is_basic_land_type(subtype: &str) -> bool {
    LAND_SUBTYPES[..5].contains(&subtype)
}

/// CR 205.1a, 205.3c: the card type `subtype` is correlated with, among
/// the represented card types.
pub fn subtype_card_type(subtype: &str) -> Option<crate::CardKind> {
    use crate::CardKind;
    let listed = |list: &str| list.split_ascii_whitespace().any(|s| s == subtype);
    if is_creature_subtype(subtype) {
        Some(CardKind::Creature)
    } else if is_land_subtype(subtype) {
        Some(CardKind::Land)
    } else if listed(ARTIFACT_SUBTYPES) {
        Some(CardKind::Artifact)
    } else if listed(ENCHANTMENT_SUBTYPES) {
        Some(CardKind::Enchantment)
    } else if listed(SPELL_SUBTYPES) {
        Some(CardKind::Instant)
    } else {
        None
    }
}
