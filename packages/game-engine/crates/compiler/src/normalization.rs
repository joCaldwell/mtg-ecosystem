//! Source-text integrity normalization, matching parser revision 327b3360.
//! This never interprets an ability or discards a source clause.
use super::{Failure, invalid};
use regex::Regex;
use std::{
    collections::BTreeMap,
    sync::{LazyLock, Mutex},
};
fn regex(pattern: &str) -> Regex {
    static CACHE: LazyLock<Mutex<BTreeMap<String, Regex>>> =
        LazyLock::new(|| Mutex::new(BTreeMap::new()));
    let mut cache = CACHE.lock().expect("regex cache");
    cache
        .entry(pattern.into())
        .or_insert_with(|| Regex::new(pattern).expect("normalization pattern"))
        .clone()
}

fn normalize_punctuation(text: &str) -> String {
    text.replace('’', "'")
        .replace(['“', '”'], "\"")
        .replace('−', "-")
        .replace('–', "—")
}
fn normalize_self_names(text: &str, name: &str, type_line: &str) -> String {
    fn replace(text: &str, name: &str, short: bool) -> String {
        if name.is_empty() {
            return text.into();
        }
        let word =
            |c: char| c.is_alphanumeric() || c == '_' || ('\u{0300}'..='\u{036f}').contains(&c);
        let mut result = String::new();
        let mut copied = 0;
        for (start, matched) in text.match_indices(name) {
            let end = start + matched.len();
            let before = &text[..start];
            let after = &text[end..];
            if before.chars().next_back().is_some_and(word)
                || after.chars().next().is_some_and(word)
            {
                continue;
            }
            let lower = before.to_lowercase();
            if regex(r#"(?i)\b(?:named|partner\s+with)\s+"?[^.,:;\n"]*$"#).is_match(before)
                && !lower.ends_with("named ")
            {
                continue;
            }
            if regex(r"(?i)\bcreates?\s+$").is_match(before)
                && regex(r"^[^.:\n]*,\s+(?:a|an)\s+").is_match(after)
            {
                continue;
            }
            if lower.ends_with("named \"") {
                continue;
            }
            let literal = lower.ends_with("named ");
            if short {
                let guarded = regex(
                    r#"(?i)\b(?:named|partner\s+with|a|an|another|each|every|all|other)\s+"?$"#,
                )
                .is_match(before)
                    || (regex(r"(?i)\btarget\s+$").is_match(before)
                        && !regex(r"(?i)\bthat\s+target\s+$").is_match(before))
                    || regex(r"(?i)^\s+(?:planeswalkers?|spells?|cards?|creatures?|tokens?)\b")
                        .is_match(after);
                let head = before.rsplit('\n').next().unwrap_or(before);
                let label = after
                    .find(" — ")
                    .filter(|dash| after.find('\n').is_none_or(|end| *dash < end))
                    .is_some_and(|dash| {
                        let label = format!("{head}{name}{}", &after[..dash]);
                        !head.contains('—')
                            && !label.contains(['.', ',', ':', ';', '{'])
                            && label.split_whitespace().count() <= 4
                    });
                if guarded || label {
                    continue;
                }
            }
            result.push_str(&text[copied..start]);
            if literal {
                result.push_str(&serde_json::to_string(name).expect("name JSON"));
            } else {
                result.push('~');
            }
            copied = end;
        }
        result.push_str(&text[copied..]);
        result
    }
    let mut text = replace(text, name, false);
    if let Some(rest) = name.strip_prefix("The ") {
        text = replace(&text, &format!("the {rest}"), false);
    }
    let types = type_line.split('—').next().unwrap_or(type_line);
    let subtypes = type_line.split_once('—').map_or("", |(_, s)| s);
    let legendary_character = types.split_whitespace().any(|s| s == "Legendary")
        && types
            .split_whitespace()
            .any(|s| matches!(s, "Creature" | "Planeswalker"));
    let short = if let Some((short, _)) = name.split_once(',') {
        Some(short.trim())
    } else if types.split_whitespace().any(|s| s == "Planeswalker") {
        name.split_whitespace()
            .next()
            .filter(|p| *p != name && subtypes.split_whitespace().any(|s| s == *p))
    } else if legendary_character {
        [name.find(" the "), name.find(" of ")].into_iter().flatten().min().map(|i| &name[..i]).or_else(|| {
            let first = name.split_whitespace().next()?;
            let subject = regex(&format!(r"\b(?:When|Whenever) {} (?:enters|attacks|dies|leaves|blocks|deals|becomes|is|transforms|or|and)\b", regex::escape(first)));
            (name.contains(' ') && first.chars().next().is_some_and(char::is_uppercase) && !subtypes.split_whitespace().any(|s| s == first) && subject.is_match(&text)).then_some(first)
        })
    } else {
        None
    };
    if let Some(short) = short {
        text = replace(&text, short, true);
    }
    if legendary_character {
        text = text
            .split('\n')
            .map(pronouns)
            .collect::<Vec<_>>()
            .join("\n");
    }
    text
}

fn pronouns(line: &str) -> String {
    // Port of the archive's source-pronouns.ts. Literal names are protected
    // separately; an unanchored or ambiguous pronoun leaves the line intact.
    let re = regex(r"(?i)\b(?:he's|she's|himself|herself|him|her|his|he|she)\b");
    let word = |c: char| c.is_alphanumeric() || matches!(c, '_' | '\'' | '-');
    let neuter = regex(r"(?i)\b(?:it|its|it's|itself)\b").is_match(line);
    let nouns = regex(r"[\p{L}][\p{L}'-]*");
    let mut result = String::new();
    let mut copied = 0;
    let mut anchor = 0;
    for m in re.find_iter(line) {
        let before = &line[..m.start()];
        let after = &line[m.end()..];
        if before.chars().next_back().is_some_and(word) || after.chars().next().is_some_and(word) {
            continue;
        }
        let sentence_start =
            before.is_empty() || before.ends_with('"') || regex(r"[.:—•]\s+$").is_match(before);
        if m.as_str() != m.as_str().to_lowercase() && !sentence_start {
            return line.into();
        }
        if let Some(i) = before.rfind('~') {
            anchor = anchor.max(i + 1);
        }
        if let Some(i) = before.rfind("this card") {
            anchor = anchor.max(i + 9);
        }
        if anchor == 0 {
            return line.into();
        }
        let role = match m.as_str().to_lowercase().as_str() {
            "he" | "she" => 0,
            "him" => 1,
            "his" => 2,
            "he's" | "she's" => 3,
            "himself" | "herself" => 4,
            "her" => {
                let next = nouns
                    .find(after.trim_start())
                    .filter(|m| m.start() == 0)
                    .map(|m| m.as_str().to_lowercase());
                match next.as_deref() {
                    Some(
                        "owner's" | "owners'" | "controller's" | "power" | "toughness" | "name"
                        | "other" | "sneak" | "loyalty" | "additional" | "mana" | "base",
                    ) => 2,
                    Some(
                        "to" | "into" | "onto" | "and" | "for" | "from" | "deal" | "unless"
                        | "this" | "is" | "equal",
                    ) => 1,
                    None if after
                        .trim_start()
                        .chars()
                        .next()
                        .is_none_or(|c| matches!(c, '.' | ',' | ';' | ':' | '"')) =>
                    {
                        1
                    }
                    _ => return line.into(),
                }
            }
            _ => unreachable!(),
        };
        let contested = neuter
            || nouns.find_iter(&line[anchor..m.start()]).any(|m| {
                let raw = m.as_str();
                matches!(
                    raw.to_lowercase().as_str(),
                    "target"
                        | "artifact"
                        | "battle"
                        | "creature"
                        | "enchantment"
                        | "instant"
                        | "kindred"
                        | "tribal"
                        | "land"
                        | "planeswalker"
                        | "sorcery"
                        | "permanent"
                        | "spell"
                        | "card"
                        | "token"
                        | "ability"
                        | "source"
                ) || NORMALIZATION_SUBTYPES.split('|').any(|s| s == raw)
            });
        let replacement = if contested {
            ["~", "~", "~'s", "~ is", "~"][role]
        } else {
            ["it", "it", "its", "it's", "itself"][role]
        };
        result.push_str(&line[copied..m.start()]);
        if sentence_start
            && m.as_str() != m.as_str().to_lowercase()
            && !replacement.starts_with('~')
        {
            result.push('I');
            result.push_str(&replacement[1..]);
        } else {
            result.push_str(replacement);
        }
        copied = m.end();
        anchor = copied;
    }
    result.push_str(&line[copied..]);
    result
}
// CR 205.3 subtype vocabulary, copied from the pinned parser vocab.ts solely
// for its source-pronoun disambiguation; this is not card-specific lowering.
const NORMALIZATION_SUBTYPES: &str = "Attraction|Blood|Bobblehead|Book|Clue|Contraption|Equipment|Food|Fortification|Gold|Heartwood|Incubator|Infinity|Junk|Lander|Map|Mutagen|Powerstone|Spacecraft|Stone|Treasure|Vehicle|Vibranium|Aura|Background|Cartouche|Case|Class|Curse|Plan|Role|Room|Rune|Saga|Shard|Shrine|Cave|Desert|Forest|Gate|Island|Lair|Locus|Mine|Mountain|Plains|Planet|Power-Plant|Sphere|Swamp|Tower|Town|Urza's|Ajani|Aminatou|Angrath|Arlinn|Ashiok|Bahamut|Basri|Bolas|Calix|Chandra|Comet|Dack|Dakkon|Daretti|Davriel|Dellian|Dihada|Domri|Dovin|Ellywick|Elminster|Elspeth|Estrid|Freyalise|Garruk|Gideon|Grist|Guff|Huatli|Jace|Jared|Jaya|Jeska|Kaito|Karn|Kasmina|Kaya|Kiora|Koth|Liliana|Lolth|Lukka|Minsc|Mordenkainen|Nahiri|Narset|Niko|Nissa|Nixilis|Oko|Quintorius|Ral|Rowan|Saheeli|Samut|Sarkhan|Serra|Sivitri|Sorin|Szat|Tamiyo|Tasha|Teferi|Teyo|Tezzeret|Tibalt|Tyvar|Ugin|Urza|Venser|Vivien|Vraska|Vronos|Will|Windgrace|Wrenn|Xenagos|Yanggu|Yanling|Zariel|Adventure|Arcane|Lesson|Omen|Trap|Time Lord|Advisor|Aetherborn|Alien|Ally|Angel|Antelope|Ape|Archer|Archon|Armadillo|Army|Artificer|Assassin|Assembly-Worker|Astartes|Atog|Aurochs|Avatar|Azra|Badger|Balloon|Barbarian|Bard|Basilisk|Bat|Bear|Beast|Beaver|Beeble|Beholder|Berserker|Bird|Bison|Blinkmoth|Boar|Bringer|Brushwagg|Camarid|Camel|Capybara|Caribou|Carrier|Cat|Centaur|Child|Chimera|Citizen|Cleric|Clown|Cockatrice|Construct|Coward|Coyote|Crab|Crocodile|C'tan|Custodes|Cyberman|Cyclops|Dalek|Dauthi|Demigod|Demon|Deserter|Detective|Devil|Dinosaur|Djinn|Doctor|Dog|Dragon|Drake|Dreadnought|Drix|Drone|Druid|Dryad|Dwarf|Echidna|Efreet|Egg|Elder|Eldrazi|Elemental|Elephant|Elf|Elk|Employee|Eternal|Eye|Faerie|Ferret|Fish|Flagbearer|Fox|Fractal|Frog|Fungus|Gamer|Gamma|Gargoyle|Germ|Giant|Giraffe|Gith|Glimmer|Gnoll|Gnome|Goat|Goblin|God|Golem|Gorgon|Graveborn|Gremlin|Griffin|Guest|Hag|Halfling|Hamster|Harpy|Hedgehog|Hellion|Hero|Hippo|Hippogriff|Homarid|Homunculus|Horror|Horse|Human|Hydra|Hyena|Illusion|Imp|Incarnation|Inhuman|Inkling|Inquisitor|Insect|Jackal|Jellyfish|Juggernaut|Kangaroo|Kavu|Kirin|Kithkin|Knight|Kobold|Kor|Kraken|Kree|Llama|Lamia|Lammasu|Leech|Lemur|Leviathan|Lhurgoyf|Licid|Lizard|Lobster|Manticore|Masticore|Mercenary|Merfolk|Metathran|Minion|Minotaur|Mite|Mole|Monger|Mongoose|Monk|Monkey|Moogle|Moonfolk|Mount|Mouse|Mutant|Myr|Mystic|Nautilus|Necron|Nephilim|Nightmare|Nightstalker|Ninja|Noble|Noggle|Nomad|Nymph|Octopus|Ogre|Ooze|Orb|Orc|Orgg|Otter|Ouphe|Ox|Oyster|Pangolin|Peasant|Pegasus|Pentavite|Performer|Pest|Phelddagrif|Phoenix|Phyrexian|Pilot|Pincher|Pirate|Plant|Platypus|Porcupine|Possum|Praetor|Primarch|Prism|Processor|Qu|Rabbit|Raccoon|Ranger|Rat|Rebel|Reflection|Rhino|Rigger|Robot|Rogue|Sable|Salamander|Samurai|Sand|Saproling|Satyr|Scarecrow|Scientist|Scion|Scorpion|Scout|Sculpture|Seal|Serf|Serpent|Servo|Shade|Shaman|Shapeshifter|Shark|Sheep|Shi'ar|Siren|Skeleton|Skrull|Skunk|Slith|Sliver|Sloth|Slug|Snail|Snake|Soldier|Soltari|Sorcerer|Spawn|Specter|Spellshaper|Sphinx|Spider|Spike|Spirit|Splinter|Sponge|Spy|Squid|Squirrel|Starfish|Surrakar|Survivor|Symbiote|Synth|Tentacle|Tetravite|Thalakos|Thopter|Thrull|Tiefling|Toy|Treefolk|Trilobite|Triskelavite|Troll|Turtle|Tyranid|Unicorn|Utrom|Vampire|Varmint|Vedalken|Villain|Volver|Wall|Walrus|Warlock|Warrior|Weasel|Weird|Werewolf|Whale|Wizard|Wolf|Wolverine|Wombat|Worm|Wraith|Wurm|Yeti|Zombie|Zubera|Undercity|Siege";

/// The archive quotes catalog-resolved literal names. Quotes in these explicit
/// name positions are lexical delimiters, not ability text. Decode only that
/// literal, preserving its complete name and every surrounding byte. Applying
/// this to both sides avoids requiring a second, unpinned name catalog.
pub(super) fn literal_spelling(text: &str) -> String {
    let re = regex(r#"(?i)\b(named|partner\s+with|creates?)\s+("(?:[^"\\]|\\.)*")"#);
    re.replace_all(text, |c: &regex::Captures<'_>| {
        let name: String = match serde_json::from_str(&c[2]) {
            Ok(name) => name,
            Err(_) => return c[0].to_owned(),
        };
        // Token names immediately after create are catalog literals only in
        // the printed "create NAME, a ..." form, as in the parser adapter.
        if c[1].to_lowercase().starts_with("create")
            && !regex(r"^,\s+(?:a|an)\s+")
                .is_match(&text[c.get(0).expect("whole literal match").end()..])
        {
            return c[0].to_owned();
        }
        c[0].replacen(&c[2], &name, 1)
    })
    .into_owned()
}

pub(super) fn source_lines(
    text: &str,
    name: &str,
    type_line: &str,
) -> Result<Vec<String>, Failure> {
    // Match parser 327b3360 normalize.ts: punctuation, balanced reminder text,
    // and case-sensitive, Unicode word-bounded self names in every position.
    // CR 201.5 / 207.2a: https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    let text = normalize_punctuation(text);
    let name = normalize_punctuation(name);
    let mut normalized = String::new();
    let mut depth = 0_u32;
    for c in text.chars() {
        match c {
            '(' => {
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| invalid("source.oracle_text", "parenthesis depth limit"))?;
                if depth == 1 {
                    normalized.push(' ');
                }
            }
            ')' if depth > 0 => depth -= 1,
            ')' => return Err(invalid("source.oracle_text", "unbalanced reminder text")),
            '\n' => normalized.push('\n'),
            c if depth == 0 => normalized.push(c),
            _ => {}
        }
    }
    if depth != 0 {
        return Err(invalid("source.oracle_text", "unbalanced reminder text"));
    }
    let normalized = normalize_self_names(&normalized, &name, type_line);
    let lines: Vec<String> = normalized
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect();
    // Block boundaries match parseAbilityBlocks in parser/index.ts at 327b3360.
    // Group all options, including Spree/pawprints and dice table rows; semantic
    // support is decided by the strict AST mirror, never by deleting an option.
    let dice = regex(
        r"(?i)^(?:•\s*)?-?\d+(?:\s*(?:[—-]\s*-?\d+|\+|or\s+(?:less|lower|fewer|higher|greater|more|\d+)))*\s*(?:\||—)",
    );
    let mut blocks = vec![];
    let mut i = 0;
    while i < lines.len() {
        let mut block = lines[i].clone();
        let paid = lines[i].eq_ignore_ascii_case("spree");
        let mut j = i + 1;
        if j < lines.len() && lines[i] != "Station" && dice.is_match(&lines[j]) {
            while j < lines.len() && dice.is_match(&lines[j]) {
                block.push('\n');
                block.push_str(&lines[j]);
                j += 1;
            }
        } else {
            while j < lines.len()
                && (lines[j].starts_with('•')
                    || lines[j].starts_with("{P}")
                    || (paid && lines[j].starts_with('+')))
            {
                block.push('\n');
                block.push_str(&lines[j]);
                j += 1;
            }
        }
        blocks.push(literal_spelling(&block));
        i = j;
    }
    Ok(blocks)
}
