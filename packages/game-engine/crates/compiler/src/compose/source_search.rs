//! Rust source authority for constrained searches of your own library
//! (CR 201.2b, 608.2c, 701.23a–b, 701.23h, 701.24a).
//!
//! A search finds a group of cards with different names, or distinct cards
//! filling individual printed slots. The same group is revealed if directed,
//! moved together to one destination, and followed by one shuffle. The
//! search is an entire spell, activated ability or self-entry trigger;
//! sibling abilities have their own source obligations. The activation
//! admits fixed mana, tapping and one sacrifice, in that order.
//!
//! Both cuts check the complete block against exact grammar and the typed
//! archive tree. R6 additionally proves the node, field and fact locations.
//! The archived binder has no authority over the claimed block.
//!
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
mod proof;

use super::*;
use crate::ingest::evidence::{BlockEvidence, CardEvidence};
use crate::reference_trace::{self, Selection, SourceChoice, SourceSelection};
use proof::Proof;
use serde_json::{Value, json};

pub(super) type Searches = BTreeMap<usize, CheckedSearch>;

enum Header {
    Spell,
    Activated(Vec<SourceCost>),
    Enters(Value),
}

pub(super) struct CheckedSearch {
    header: Header,
    group: kir::SearchGroup,
    filter: Pred,
    maximum: i64,
    reveal: bool,
    destination: SearchDestination,
    selections: Vec<SourceSelection>,
}

enum SearchDestination {
    Hand,
    Graveyard,
    Battlefield { tapped: bool },
}

fn candidate(ability: &Value) -> bool {
    let search = &ability["effects"][0]["steps"][0];
    search["effect"] == "search"
        && (search["for"]["ref"] == "card-extraction-group"
            || search["for"]["groupConstraints"]
                == json!([
                    {"constraint":"pairwise-different","property":"name"}
                ]))
}

pub(super) fn discover(
    archive: &AcceptedArchive,
    card: &CardSource,
    evidence: Option<&CardEvidence>,
) -> Result<Searches, Failure> {
    let [(face, parsed)] = card.faces.as_slice() else {
        return Ok(BTreeMap::new());
    };
    if !matches!(archive.cut, ArchiveCut::R5 | ArchiveCut::R6)
        || !parsed.abilities.get().contains("\"search\"")
    {
        return Ok(BTreeMap::new());
    }
    let abilities: Vec<Value> = decode(parsed.abilities.get(), "abilities")?;
    if !abilities.iter().any(candidate) {
        return Ok(BTreeMap::new());
    }
    let lines = source_lines(&face.oracle_text, &face.name, &face.type_line)?;
    if lines.len() != parsed.lines.len() || abilities.len() != lines.len() {
        return Err(invalid(
            "lines",
            "source, line and aggregate counts disagree",
        ));
    }
    let types = type_line::parse(&face.type_line, false)?;
    let card = Card {
        kind: types.kind,
        additional_types: &types.additional_types,
        subtypes: &types.subtypes,
    };
    let mut claims = BTreeMap::new();
    for (index, ability) in abilities
        .iter()
        .enumerate()
        .filter(|(_, ability)| candidate(ability))
    {
        let line = &lines[index];
        if decode::<Value>(parsed.lines[index].ability.get(), "lines.ability")? != *ability
            || normalization::literal_spelling(&parsed.lines[index].text) != *line
        {
            return Err(invalid(
                "lines",
                "source, line and aggregate ability disagree",
            ));
        }
        // The generated AST rejects unknown fields before the source grammar
        // compares every expected field, including omitted alternatives.
        typed::<ast::Ability>(ability, "abilities")?;
        let printed = if archive.cut == ArchiveCut::R6 {
            Some(
                evidence
                    .ok_or_else(|| invalid("printed", "missing verified card evidence"))?
                    .block(0, index, "/effects/0/steps/0")?,
            )
        } else {
            None
        };
        claims.insert(index, parse(ability, line, index, &card, printed.as_ref())?);
    }
    Ok(claims)
}

fn parse(
    ability: &Value,
    line: &str,
    index: usize,
    card: &Card<'_>,
    block: Option<&BlockEvidence<'_>>,
) -> Result<CheckedSearch, Failure> {
    if let Some(block) = block {
        let (line_index, text) = block.single_line().ok_or_else(|| block.missing(""))?;
        if normalization::literal_spelling(text) != line {
            return Err(block.missing("").into());
        }
        block
            .check_family(|coverage| {
                let mut proof = Proof::new(ability, text, line_index, Some(coverage));
                let search = parse_search(&mut proof, card).map_err(|path| block.missing(&path))?;
                proof.cover_facts(block)?;
                Ok(search)
            })
            .map_err(Failure::from)
    } else {
        let mut proof = Proof::new(ability, line, 0, None);
        parse_search(&mut proof, card).map_err(|path| {
            unsupported(
                &format!("faces/0/blocks/{index}{path}"),
                "source:grouped-search:printed",
            )
        })
    }
}

type Gap = String;

fn parse_search(proof: &mut Proof<'_, '_, '_, '_>, card: &Card<'_>) -> Result<CheckedSearch, Gap> {
    let (header, prefix, mut selections) = parse_header(proof, card)?;
    let search_path = "/effects/0/steps/0";
    let verb = proof.take("search", search_path)?;
    proof.mark("/kind", &prefix["kind"], verb)?;
    proof.mark(&format!("{search_path}/effect"), "search", verb)?;
    let zones = proof.take("your library", search_path)?;
    let zone = format!("{search_path}/zones/of/0");
    proof.mark(&format!("{search_path}/zones/selection"), "all", zones)?;
    proof.mark(&format!("{zone}/owner"), "your", zones.first(1))?;
    proof.mark(&format!("{zone}/zone"), "library", zones.last(1))?;
    let for_word = proof.take("for", search_path)?;
    proof.mark(&format!("{search_path}/effect"), "search", for_word)?;
    proof.controller(&format!("{search_path}/who/player"))?;
    selections.push(SourceSelection {
        path: format!("{search_path}/who"),
        selection: Selection::SpellController,
    });

    let find_path = format!("{search_path}/for");
    let (group, filter, maximum, find) = if proof.peek("up") {
        different_names(proof, &find_path)?
    } else {
        separate_slots(proof, &find_path)?
    };
    selections.push(SourceSelection {
        path: find_path,
        selection: Selection::SearchGroup,
    });
    let search = json!({
        "effect":"search", "who":{"player":"you"},
        "zones":{"selection":"all","of":[{"zone":"library","owner":"your"}]}, "for":find
    });

    let separate_sentence = proof.peek(".");
    let mut current = if separate_sentence {
        proof.period("/effects/0")?;
        0
    } else {
        proof.structure(",", "/effects/0")?;
        1
    };
    let sentence = if separate_sentence { 1 } else { 0 };
    let mut tail = Vec::new();
    let reveal = proof.peek("reveal");
    if reveal {
        let path = format!("/effects/{sentence}/steps/{current}");
        let verb = proof.take("reveal", &path)?;
        proof.mark(&format!("{path}/effect"), "reveal", verb)?;
        proof.controller(&format!("{path}/who/player"))?;
        let what = found_object(proof, &path, search_path, &mut selections)?;
        tail.push(json!({"effect":"reveal","who":{"player":"you"},"what":what}));
        proof.structure(",", &format!("/effects/{sentence}"))?;
        current += 1;
    }

    let path = format!("/effects/{sentence}/steps/{current}");
    let put = proof.take("put", &path)?;
    proof.mark(&format!("{path}/effect"), "move-zone", put)?;
    let what = found_object(proof, &path, search_path, &mut selections)?;
    let (destination, to, tapped) = if proof.peek("onto") {
        let phrase = proof.take("onto the battlefield", &path)?;
        proof.mark(&format!("{path}/effect"), "move-zone", phrase.first(1))?;
        proof.mark(&format!("{path}/to/zone"), "battlefield", phrase.last(2))?;
        let tapped = if proof.peek("tapped") {
            let run = proof.take("tapped", &path)?;
            proof.mark(&format!("{path}/tapped"), true, run)?;
            true
        } else {
            false
        };
        (
            SearchDestination::Battlefield { tapped },
            json!({"zone":"battlefield"}),
            tapped,
        )
    } else {
        let into = proof.take("into", &path)?;
        proof.mark(&format!("{path}/effect"), "move-zone", into)?;
        let zone = if proof.peek_phrase("your hand") {
            "hand"
        } else {
            "graveyard"
        };
        let phrase = proof.take(&format!("your {zone}"), &path)?;
        proof.mark(&format!("{path}/to/owner"), "your", phrase.first(1))?;
        proof.mark(&format!("{path}/to/zone"), zone, phrase.last(1))?;
        let destination = if zone == "hand" {
            SearchDestination::Hand
        } else {
            SearchDestination::Graveyard
        };
        (destination, json!({"zone":zone,"owner":"your"}), false)
    };
    let mut movement = json!({"effect":"move-zone","what":what,"to":to});
    if tapped {
        movement["tapped"] = json!(true);
    }
    tail.push(movement);
    proof.structure(",", &format!("/effects/{sentence}"))?;
    let path = format!("/effects/{sentence}/steps/{}", current + 1);
    let shuffle = proof.take("then shuffle", &path)?;
    proof.structure_run(shuffle.first(1), &format!("/effects/{sentence}"))?;
    proof.mark(&format!("{path}/effect"), "shuffle", shuffle.last(1))?;
    proof.controller(&format!("{path}/who/player"))?;
    proof.controller(&format!("{path}/library/owner/player"))?;
    proof.zone_default(&format!("{path}/library/zone"), "library")?;
    tail.push(json!({"effect":"shuffle","who":{"player":"you"},"library":{"zone":"library","owner":{"player":"you"}}}));
    proof.period(&format!("/effects/{sentence}"))?;
    let mut expected = prefix;
    expected["effects"] = if separate_sentence {
        json!([{"steps":[search]},{"steps":tail}])
    } else {
        tail.insert(0, search);
        json!([{"steps":tail}])
    };
    proof.complete(&expected)?;

    Ok(CheckedSearch {
        header,
        group,
        filter,
        maximum,
        reveal,
        destination,
        selections,
    })
}

fn parse_header(
    proof: &mut Proof<'_, '_, '_, '_>,
    card: &Card<'_>,
) -> Result<(Header, Value, Vec<SourceSelection>), Gap> {
    if proof.peek("search") {
        return Ok((Header::Spell, json!({"kind":"spell"}), vec![]));
    }
    if proof.peek("when") {
        let path = "/trigger";
        let head = proof.take("when this creature enters", path)?;
        proof.mark("/trigger/trigger", "enters", head.last(1))?;
        proof.mark("/trigger/what/ref", "this", head.within(1, 1))?;
        proof.mark("/trigger/what/noun", "creature", head.within(2, 1))?;
        proof.structure_run(head.first(1), path)?;
        proof.clause_comma(path)?;
        let event = json!({"trigger":"enters","what":{"ref":"this","noun":"creature"}});
        return Ok((
            Header::Enters(event.clone()),
            json!({"kind":"triggered","trigger":event}),
            vec![SourceSelection {
                path: "/trigger/what".into(),
                selection: Selection::Source,
            }],
        ));
    }
    let mut costs = Vec::new();
    let mut symbols = Vec::new();
    while let Some(symbol) = proof.mana_symbol() {
        symbols.push(symbol.to_ascii_uppercase());
    }
    if !symbols.is_empty() {
        let path = "/costs/0";
        let run = proof.back(symbols.len() as u32);
        proof.mark(&format!("{path}/cost"), "mana", run)?;
        proof.mark(&format!("{path}/symbols"), &symbols, run)?;
        costs.push(json!({"cost":"mana","symbols":symbols}));
        proof.cost_comma(path)?;
    }
    let path = format!("/costs/{}", costs.len());
    let tap = proof.take("{t}", &path)?;
    proof.mark(&format!("{path}/cost"), "tap-self", tap)?;
    costs.push(json!({"cost":"tap-self"}));
    proof.cost_comma(&path)?;

    let path = format!("/costs/{}", costs.len());
    let sacrifice = proof.take("sacrifice", &path)?;
    proof.mark(&format!("{path}/cost"), "sacrifice", sacrifice)?;
    let (what, selections) = if proof.peek("this") {
        let referent = proof.take("this", &path)?;
        let (noun, noun_run) = proof.word(&path)?;
        if !card.is_self(&noun) {
            return Err(path);
        }
        proof.mark(&format!("{path}/what/ref"), "this", referent)?;
        proof.mark(&format!("{path}/what/noun"), &noun, noun_run)?;
        (
            json!({"ref":"this","noun":noun}),
            vec![SourceSelection {
                path: format!("{path}/what"),
                selection: Selection::Source,
            }],
        )
    } else {
        let article = proof.take("a", &path)?;
        let noun = proof.take("creature", &path)?;
        proof.mark(&format!("{path}/what/ref"), "filter", article.join(noun))?;
        proof.mark(&format!("{path}/what/filter/types"), ["creature"], noun)?;
        proof.count(&format!("{path}/what/count"), 1, article)?;
        (
            json!({"ref":"filter","filter":{"types":["creature"]},"count":{"amount":"fixed","value":1}}),
            vec![SourceSelection {
                path: format!("{path}/what"),
                selection: Selection::ActivationSacrifice,
            }],
        )
    };
    costs.push(json!({"cost":"sacrifice","what":what}));
    proof.cost_colon(&path)?;
    let terms = serde_json::from_value(json!(costs)).map_err(|_| "/costs".to_owned())?;
    Ok((
        Header::Activated(terms),
        json!({"kind":"activated","costs":costs}),
        selections,
    ))
}

fn different_names(
    proof: &mut Proof<'_, '_, '_, '_>,
    path: &str,
) -> Result<(kir::SearchGroup, Pred, i64, Value), Gap> {
    let upto = proof.take("up to", path)?;
    proof.mark(&format!("{path}/upTo"), true, upto)?;
    let (number, run) = proof.word(path)?;
    let maximum = i64::from(
        word_number(&number)
            .or_else(|| number.parse().ok())
            .filter(|&n| n > 0)
            .ok_or_else(|| path.to_owned())?,
    );
    proof.count(&format!("{path}/count"), maximum, run)?;
    let filter_path = format!("{path}/filter");
    let (filter, pred) = card_filter(proof, &filter_path, "cards")?;
    let constraint = proof.take("with different names", path)?;
    let group = format!("{path}/groupConstraints/0");
    proof.mark(
        &format!("{group}/constraint"),
        "pairwise-different",
        constraint,
    )?;
    proof.mark(&format!("{group}/property"), "name", constraint.last(1))?;
    proof.mark(&format!("{path}/ref"), "filter", upto)?;
    Ok((
        kir::SearchGroup::DifferentNames,
        pred,
        maximum,
        json!({
            "ref":"filter","filter":filter,"upTo":true,
            "groupConstraints":[{"constraint":"pairwise-different","property":"name"}],
            "count":{"amount":"fixed","value":maximum}
        }),
    ))
}

fn separate_slots(
    proof: &mut Proof<'_, '_, '_, '_>,
    path: &str,
) -> Result<(kir::SearchGroup, Pred, i64, Value), Gap> {
    let mut slots = Vec::new();
    let mut members = Vec::new();
    let start = proof.position();
    loop {
        if members.len() == 8 {
            return Err(path.to_owned());
        }
        let member = format!("{path}/members/{}", members.len());
        let article = proof.either("a", "an", &member)?;
        proof.count(&format!("{member}/count"), 1, article)?;
        let (filter, pred) = card_filter(proof, &format!("{member}/filter"), "card")?;
        slots.push(pred);
        members.push(json!({"filter":filter,"count":{"amount":"fixed","value":1}}));
        if proof.peek("and") {
            let and = if proof.peek_phrase("and/or") {
                proof.take("and/or", path)?
            } else {
                proof.take("and", path)?
            };
            let combine = if and.width() == 3 {
                "any-nonempty"
            } else {
                "all"
            };
            let member = format!("{path}/members/{}", members.len());
            let article = proof.either("a", "an", &member)?;
            proof.count(&format!("{member}/count"), 1, article)?;
            let (filter, pred) = card_filter(proof, &format!("{member}/filter"), "card")?;
            slots.push(pred);
            members.push(json!({"filter":filter,"count":{"amount":"fixed","value":1}}));
            proof.mark(&format!("{path}/combine"), combine, and)?;
            let full = proof.since(start);
            proof.mark(&format!("{path}/ref"), "card-extraction-group", full)?;
            proof.mark(&format!("{path}/allocation"), "distinct-objects", full)?;
            let maximum = slots.len() as i64;
            return Ok((
                kir::SearchGroup::Separate { slots },
                Pred(vec![]),
                maximum,
                json!({
                    "ref":"card-extraction-group","combine":combine,
                    "allocation":"distinct-objects","members":members,
                }),
            ));
        }
        proof.structure(",", path)?;
    }
}

fn card_filter(
    proof: &mut Proof<'_, '_, '_, '_>,
    path: &str,
    noun: &str,
) -> Result<(Value, Pred), Gap> {
    let (first, run) = proof.word(path)?;
    let mut filter = json!({"cls":"card"});
    match first.as_str() {
        "white" | "blue" | "black" | "red" | "green" => {
            proof.mark(&format!("{path}/colors"), [&first], run)?;
            filter["colors"] = json!([first]);
        }
        "artifact" | "creature" | "enchantment" | "instant" | "land" | "planeswalker"
        | "sorcery" => {
            proof.mark(&format!("{path}/types"), [&first], run)?;
            filter["types"] = json!([first]);
        }
        _ => {
            // The frozen grammar spells subtypes with their canonical initial
            // capitals. The shared card-predicate lowerer checks the domain.
            let subtype = proof.spelling(run);
            if kir::subtype_card_type(subtype).is_none() {
                return Err(path.to_owned());
            }
            proof.mark(&format!("{path}/subtypes"), [subtype], run)?;
            filter["subtypes"] = json!([subtype]);
            if proof.peek("creature") {
                let typed = proof.take("creature", path)?;
                proof.mark(&format!("{path}/types"), ["creature"], typed)?;
                filter["types"] = json!(["creature"]);
            }
        }
    }
    let noun = proof.take(noun, path)?;
    proof.mark(&format!("{path}/cls"), "card", noun)?;
    let typed = typed::<ast::ObjectFilter>(&filter, "filter").map_err(|_| path.to_owned())?;
    let pred = lower_card_filter(&typed).map_err(|_| path.to_owned())?;
    Ok((filter, pred))
}

fn found_object(
    proof: &mut Proof<'_, '_, '_, '_>,
    path: &str,
    search_path: &str,
    selections: &mut Vec<SourceSelection>,
) -> Result<Value, Gap> {
    let what = format!("{path}/what");
    let object = if proof.peek("them") {
        let run = proof.take("them", &what)?;
        proof.mark(&format!("{what}/ref"), "them", run)?;
        json!({"ref":"them"})
    } else {
        let run = proof.take("those cards", &what)?;
        proof.mark(&format!("{what}/ref"), "that", run.first(1))?;
        proof.mark(&format!("{what}/plural"), true, run)?;
        proof.mark(&format!("{what}/filter/cls"), "card", run.last(1))?;
        json!({"ref":"that","filter":{"cls":"card"},"plural":true})
    };
    selections.push(SourceSelection {
        path: what,
        selection: Selection::SearchResult {
            step: search_path.to_owned(),
        },
    });
    Ok(object)
}

impl CheckedSearch {
    pub(super) fn spell(&self) -> bool {
        matches!(self.header, Header::Spell)
    }

    pub(super) fn lower(
        self,
        card: &Card<'_>,
        block: usize,
        line: &str,
        definition: &mut Definition,
        labels: &mut SourceLabels,
    ) -> Result<(), Failure> {
        let label = source_block(block)?;
        let (program, context) = match self.header {
            Header::Spell => (
                definition
                    .spell
                    .as_mut()
                    .ok_or_else(|| unsupported("compose.kind", "grouped search without a spell"))?,
                Context::Spell,
            ),
            Header::Activated(costs) => {
                let ability = activation(card, &costs, vec![], SpellTiming::Instant, block, line)?;
                definition.activated.push(ability);
                labels.activated.push(label);
                (
                    &mut definition.activated.last_mut().unwrap().program,
                    Context::Activated,
                )
            }
            Header::Enters(event) => {
                let ability = trigger(card, &event, vec![], None, block)?;
                definition.triggered.push(ability);
                labels.triggered.push(label);
                (
                    &mut definition.triggered.last_mut().unwrap().program,
                    Context::Triggered,
                )
            }
        };
        let player = PlayerExpr::you(context);
        let to = match self.destination {
            SearchDestination::Hand => Destination::OwnerHand,
            SearchDestination::Graveyard => Destination::OwnerGraveyard,
            SearchDestination::Battlefield { tapped } => Destination::Battlefield {
                controller: player.clone(),
                tapped,
            },
        };
        let search = kir::SearchSpec {
            filter: self.filter,
            maximum: Amount::fixed(self.maximum),
            to,
            group: Some(self.group),
            reveal: self.reveal,
            from: kir::SelectionZone::Library,
            optional: false,
            random: false,
        };
        program.instructions.extend(
            [
                Verb::Search {
                    player: player.clone(),
                    search,
                },
                Verb::Shuffle { player },
            ]
            .map(|verb| Instruction::Kernel {
                node: kir::Node::Do(verb),
                source_block: label,
            }),
        );
        reference_trace::record_source(|| SourceChoice {
            family: "grouped-search",
            face: 0,
            block,
            selections: self.selections,
        });
        Ok(())
    }
}
