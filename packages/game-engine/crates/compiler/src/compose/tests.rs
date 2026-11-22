use super::*;
use serde_json::json;

fn card(kind: CardKind) -> Card<'static> {
    Card {
        kind,
        additional_types: &[],
    }
}

fn costs(raw: serde_json::Value) -> Vec<SourceCost> {
    serde_json::from_value(raw).unwrap()
}

#[test]
fn cost_terms_select_one_complete_runtime_cost_and_never_drop_a_term() {
    let artifact = card(CardKind::Artifact);
    let this = |noun: &str| json!({"cost":"sacrifice","what":{"ref":"this","noun":noun}});
    assert_eq!(
        costs::lower(&artifact, &costs(json!([{"cost":"tap-self"}]))).unwrap(),
        ActivationCost::TapSource
    );
    assert_eq!(
        costs::lower(
            &artifact,
            &costs(json!([{"cost":"mana","symbols":["2"]},{"cost":"tap-self"},this("artifact")]))
        )
        .unwrap(),
        ActivationCost::Composite {
            costs: PermanentActivationCosts {
                remove_source_plus_one_counters: None,
                tap: true,
                mana: Some(mana_cost("{2}").unwrap()),
                sacrifice: true,
            }
        }
    );
    // CR 701.21a: the exact creature source keeps its dedicated cost.
    assert_eq!(
        costs::lower(&card(CardKind::Creature), &costs(json!([this("creature")]))).unwrap(),
        ActivationCost::SacrificeSource
    );
    assert!(matches!(
        costs::lower(&card(CardKind::Enchantment), &costs(json!([this("enchantment")]))).unwrap(),
        ActivationCost::Composite { costs } if costs.sacrifice && !costs.tap && costs.mana.is_none()
    ));
    // A self reference must name one of the card's own types (CR 201.5).
    assert!(costs::lower(&artifact, &costs(json!([this("creature")]))).is_err());
    let another = json!({"cost":"sacrifice","what":{"ref":"filter","filter":{"types":["creature"]},
        "count":{"amount":"fixed","value":1}}});
    assert!(matches!(
        costs::lower(
            &artifact,
            &costs(json!([{"cost":"mana","symbols":["1"]},another]))
        )
        .unwrap(),
        ActivationCost::SacrificePermanent {
            count: 1,
            mana: Some(_),
            ..
        }
    ));
    for unsupported in [
        json!([]),
        json!([{"cost":"tap-self"},{"cost":"tap-self"}]),
        json!([{"cost":"mana","symbols":["1"]},{"cost":"mana","symbols":["1"]}]),
        json!([{"cost":"mana","symbols":["C"]}]),
        json!([{"cost":"mana","symbols":["{1}"]}]),
        json!([{"cost":"tap-self"},another]),
        json!([{"cost":"tap-self"},{"cost":"pay-life","amount":{"amount":"fixed","value":1}}]),
    ] {
        assert!(
            costs::lower(&artifact, &costs(unsupported.clone())).is_err(),
            "{unsupported}"
        );
    }
}

#[test]
fn event_rows_only_add_self_entry_for_the_cards_own_type() {
    let enters = |noun: &str| -> SourceTrigger {
        serde_json::from_value(json!({"trigger":"enters","what":{"ref":"this","noun":noun}}))
            .unwrap()
    };
    assert_eq!(
        events::lower(&card(CardKind::Artifact), &enters("artifact")),
        Some(TriggerCondition::SelfEntersBattlefield)
    );
    assert_eq!(
        events::lower(&card(CardKind::Land), &enters("land")),
        Some(TriggerCondition::SelfEntersBattlefield)
    );
    // Another object's type is not a self reference.
    assert_eq!(
        events::lower(&card(CardKind::Land), &enters("artifact")),
        None
    );
    // The shared trigger lowerer keeps the spellings it already owns.
    assert_eq!(
        events::lower(&card(CardKind::Creature), &enters("creature")),
        None
    );
    let dies: SourceTrigger =
        serde_json::from_value(json!({"trigger":"dies","what":{"ref":"this","noun":"creature"}}))
            .unwrap();
    assert_eq!(events::lower(&card(CardKind::Creature), &dies), None);
}

#[test]
fn keyword_rows_reject_parameters_and_unknown_keywords() {
    let keywords =
        |raw: serde_json::Value| -> Vec<SourceKeyword> { serde_json::from_value(raw).unwrap() };
    let creature = card(CardKind::Creature);
    assert_eq!(
        keywords::lower(
            &creature,
            &keywords(json!([{"keyword":"flying"},{"keyword":"first-strike"}]))
        )
        .unwrap(),
        [KeywordAbility::Flying, KeywordAbility::FirstStrike]
    );
    assert!(keywords::lower(&creature, &[]).is_err());
    assert!(
        keywords::lower(
            &creature,
            &keywords(json!([{"keyword":"cycling","cost":[{"cost":"mana","symbols":["2"]}]}]))
        )
        .is_err()
    );
    assert!(keywords::lower(&creature, &keywords(json!([{"keyword":"cascade"}]))).is_err());
}

const R4: &str = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r4.json");
const STRIP: &str = include_str!("../../../../tests/fixtures/phase1-r4/compose-strip-mine.json");
const SEAL: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/compose-seal-of-primordium.json");
const MARSH: &str = include_str!("../../../../tests/fixtures/phase1-r4/compose-piranha-marsh.json");
const DRAGON: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/compose-destructor-dragon.json");
const PIE: &str = include_str!("../../../../tests/fixtures/phase1-r4/compose-bake-into-a-pie.json");
const CREW: &str = include_str!("../../../../tests/fixtures/phase1-r4/compose-hire-a-crew.json");
const WITNESSES: [&str; 9] = [
    include_str!("../../../../tests/fixtures/phase1-r4/compose-marble-chalice.json"),
    include_str!("../../../../tests/fixtures/phase1-r4/compose-rishadan-port.json"),
    STRIP,
    SEAL,
    include_str!("../../../../tests/fixtures/phase1-r4/compose-kamahl-pit-fighter.json"),
    include_str!("../../../../tests/fixtures/phase1-r4/compose-icatian-priest.json"),
    MARSH,
    DRAGON,
    PIE,
];

fn composed(raw: &str) -> Definition {
    let CompileResult::Lowered { definition } = compile(R4, raw) else {
        panic!("composed frozen source must lower: {:?}", compile(R4, raw));
    };
    assert!(definition.composed);
    assert!(
        definition
            .capabilities
            .contains(&Capability::ComposedAbilityV1)
    );
    *definition
}

/// Re-hash a changed archive row, keeping each line's AST equal to the aggregate.
fn changed(raw: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
    crate::tests::changed_source(raw, |row| {
        change(row);
        let result = &mut row["faces"][0]["result"];
        let abilities = result["abilities"].as_array().unwrap().clone();
        for (i, ability) in abilities.into_iter().enumerate() {
            result["lines"][i]["ability"] = ability;
        }
    })
}

#[test]
fn frozen_witnesses_lower_block_by_block_and_round_trip_executable_ir() {
    for raw in WITNESSES {
        let definition = composed(raw);
        definition.validate().unwrap();
        // Executable IR v1 round trip: the marker and capability survive, and
        // the strict reader accepts exactly what the compiler emitted.
        let catalog = Catalog {
            schema_version: CURRENT_IR_VERSION,
            rules_edition: RULES_EDITION.into(),
            rules_sha256: RULES_SHA256.into(),
            definitions: vec![definition.clone()],
        };
        let json = serde_json::to_string(&catalog).unwrap();
        assert!(json.contains("\"composed\":true"));
        assert_eq!(Catalog::read(&json).unwrap().definitions, [definition]);
    }
}

#[test]
fn deferred_binding_checks_name_the_first_block_blocker_without_admitting() {
    let archive = accepted_manifest(R4).unwrap();
    // A card-level residual: the resolution is not complete.
    let unbound = |row: &mut serde_json::Value| {
        row["resolution"]["complete"] = json!(false);
        row["resolution"]["residuals"] = json!(["diagnostic:missing-antecedent"]);
    };
    // CR 207.2c: an annotation block 1 does not print is a block blocker.
    let unprinted = |row: &mut serde_json::Value| {
        row["faces"][0]["result"]["abilities"][1]["abilityWord"] = json!("Unprinted");
    };
    let summary = |found: &Blockers| {
        (
            found.binding.as_ref().map(|f| f.path.clone()),
            found.lowering.as_ref().map(|f| f.path.clone()),
            found.block,
        )
    };
    let clean = blockers(archive, STRIP);
    assert_eq!(summary(&clean), (None, None, None));
    assert!(lower(archive, STRIP).is_ok());
    // Only the card-wide binding check fails: lowering still rejects it, and
    // the report names no block.
    let only_binding = changed(STRIP, unbound);
    assert_eq!(
        lower(archive, &only_binding).unwrap_err().path,
        "resolution"
    );
    assert_eq!(
        summary(&blockers(archive, &only_binding)),
        (Some("resolution".into()), None, None)
    );
    // Both fail: lowering is unchanged (the binding verdict comes first) and
    // the report also names the block that would fail next.
    let both = changed(STRIP, |row| {
        unbound(row);
        unprinted(row);
    });
    assert_eq!(lower(archive, &both).unwrap_err().path, "resolution");
    // The annotation no printed token produced fails the span certificate.
    assert_eq!(
        summary(&blockers(archive, &both)),
        (
            Some("resolution".into()),
            Some("resolution.spans".into()),
            Some(1)
        )
    );
    // Only the block fails: the report and lowering agree.
    let only_block = changed(STRIP, unprinted);
    let failure = lower(archive, &only_block).unwrap_err();
    let found = blockers(archive, &only_block);
    assert_eq!(summary(&found), (None, Some(failure.path), Some(1)));
    assert_eq!(found.lowering.unwrap().reason, failure.reason);
}

#[test]
fn composed_blocks_keep_their_printed_order_cost_and_operands() {
    // A land: one mana block and one tap-and-sacrifice activation.
    let strip = composed(STRIP);
    assert_eq!(strip.mana_abilities.len(), 1);
    let [ability] = strip.activated.as_slice() else {
        panic!("one activation")
    };
    assert!(matches!(
        &ability.cost,
        ActivationCost::Composite { costs }
            if costs.tap && costs.sacrifice && costs.mana.is_none()
    ));
    assert!(matches!(
        ability.program.instructions.as_slice(),
        [kernel!(Destroy { what: target_object!(0), regeneration_forbidden: _ } @ 1)]
    ));
    assert_eq!(
        ability.program.targets[0].domain,
        TargetDomain::BattlefieldPermanent
    );
    // An enchantment sacrifices itself; the self noun is the card's own type.
    let seal = composed(SEAL);
    assert!(matches!(
        &seal.activated[0].cost,
        ActivationCost::Composite { costs } if costs.sacrifice && !costs.tap && costs.mana.is_none()
    ));
    // A land's own entry replacement, entry trigger and mana ability.
    let marsh = composed(MARSH);
    assert_eq!(marsh.static_abilities[0].effect, StaticEffect::EntersTapped);
    assert_eq!(
        marsh.triggered[0].condition,
        TriggerCondition::SelfEntersBattlefield
    );
    assert_eq!(
        marsh.triggered[0].program.targets[0].domain,
        TargetDomain::AnyPlayer
    );
    let labels = marsh.source_labels.as_ref().unwrap();
    assert_eq!(
        (labels.statics.as_slice(), labels.triggered.as_slice()),
        (&[0][..], &[1][..])
    );
    assert_eq!(labels.mana, [2]);
    // A spell sequence stays in printed order.
    let pie = composed(PIE);
    assert_eq!(
        pie.spell.as_ref().unwrap().instructions,
        [
            Instruction::destroy_target(0, false, 0),
            Instruction::create_tokens(
                kir::Context::Spell,
                1,
                kir::TokenSpec::Predefined {
                    kind: PredefinedToken::Food
                },
                0
            )
        ]
    );
    // A creature's dies trigger on any noncreature permanent.
    let dragon = composed(DRAGON);
    assert_eq!(dragon.keywords, [KeywordAbility::Flying]);
    assert_eq!(dragon.triggered[0].condition, TriggerCondition::SelfDies);
}

#[test]
fn printed_text_and_ast_must_agree_on_choices_words_and_numbers() {
    let rejected = |raw: &str| !matches!(compile(R4, raw), CompileResult::Lowered { .. });
    let reworded = |raw: &str, from: &str, to: &str| {
        changed(raw, |row| {
            let text = row["source"]["oracle_text"]
                .as_str()
                .unwrap()
                .replace(from, to);
            assert_ne!(row["source"]["oracle_text"], json!(text));
            row["source"]["oracle_text"] = json!(text);
            let line = &mut row["faces"][0]["result"]["lines"][0]["text"];
            *line = json!(line.as_str().unwrap().replace(from, to));
        })
    };
    // A printed choice the AST makes mandatory, and an AST choice not printed.
    assert!(rejected(&reworded(
        PIE,
        "Create a Food token.",
        "You may create a Food token."
    )));
    assert!(rejected(&changed(PIE, |row| {
        row["faces"][0]["result"]["abilities"][0]["effects"][1]["steps"][0]["optional"] =
            json!(true);
    })));
    // A printed limit or condition the AST does not carry.
    assert!(rejected(&reworded(
        PIE,
        "Destroy target creature.",
        "Destroy up to one target creature."
    )));
    assert!(rejected(&reworded(
        PIE,
        "Create a Food token.",
        "Create a Food token if you control a Swamp."
    )));
    // A printed number the AST does not carry.
    assert!(rejected(&reworded(
        PIE,
        "Create a Food token.",
        "Create 4 Food tokens."
    )));
    // An evergreen keyword in the printed keyword list but not the AST, and
    // an ability word the block does not print.
    assert!(rejected(&changed(PIE, |row| {
        row["source"]["keywords"] = json!(["Flash"])
    })));
    assert!(rejected(&changed(PIE, |row| {
        row["faces"][0]["result"]["abilities"][0]["abilityWord"] = json!("Delirium");
    })));
}

#[test]
fn changed_sources_reject_instead_of_dropping_or_guessing() {
    let rejected = |raw: &str| !matches!(compile(R4, raw), CompileResult::Lowered { .. });
    let composition_rejects = |raw: &str| lower(accepted_manifest(R4).unwrap(), raw).is_err();
    for witness in [DRAGON, MARSH] {
        assert!(lower(accepted_manifest(R4).unwrap(), witness).is_ok());
    }
    // An unconsumed step field.
    assert!(rejected(&changed(STRIP, |row| {
        row["faces"][0]["result"]["abilities"][1]["effects"][0]["steps"][0]["unless"] = json!(true);
    })));
    // An extra cost term the runtime cannot pay atomically.
    assert!(rejected(&changed(STRIP, |row| {
        row["faces"][0]["result"]["abilities"][1]["costs"]
            .as_array_mut()
            .unwrap()
            .push(json!({"cost":"pay-life","amount":{"amount":"fixed","value":1}}));
    })));
    // A self reference naming a type the card does not have (CR 201.5).
    assert!(rejected(&changed(SEAL, |row| {
        row["faces"][0]["result"]["abilities"][0]["costs"][0]["what"]["noun"] = json!("creature");
    })));
    // An activation restriction outside the timing table.
    assert!(rejected(&changed(SEAL, |row| {
        row["faces"][0]["result"]["abilities"][0]["restriction"] =
            json!({"restriction":"timing","turn":"your"});
    })));
    // The resolution must be complete, card-wide and in every block.
    assert!(composition_rejects(&changed(DRAGON, |row| {
        row["resolution"]["complete"] = json!(false);
        row["resolution"]["residuals"] = json!(["face:unparsed"]);
    })));
    assert!(composition_rejects(&changed(DRAGON, |row| {
        row["resolution"]["complete"] = json!(false);
        row["resolution"]["faces"][0]["blocks"][1]["residuals"] = json!(["ref:missing-antecedent"]);
    })));
    // A pronoun resolved to the source instead of the block's announced target.
    assert!(composition_rejects(&changed(DRAGON, |row| {
        row["resolution"]["faces"][0]["blocks"][1]["refs"][0]["ref"] = json!({"ref":"source"});
    })));
    // A clause removed from the AST leaves its bound references dangling,
    // and an injected clause has no bound event.
    let shortened = changed(CREW, |row| {
        row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"]
            .as_array_mut()
            .unwrap()
            .pop();
    });
    assert!(lower(accepted_manifest(R4).unwrap(), CREW).is_ok());
    assert!(lower(accepted_manifest(R4).unwrap(), &shortened).is_err());
    assert!(rejected(&changed(SEAL, |row| {
        row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"]
            .as_array_mut()
            .unwrap()
            .push(json!({"effect":"draw","who":{"player":"you"},
                "amount":{"amount":"fixed","value":1}}));
    })));
    // A reference at a path the AST does not have, a "you" resolved to
    // another player's role, and a repeated reference path.
    assert!(composition_rejects(&changed(DRAGON, |row| {
        row["resolution"]["faces"][0]["blocks"][1]["refs"][0]["path"] =
            json!("/effects/0/steps/0/absent/what");
    })));
    assert!(composition_rejects(&changed(MARSH, |row| {
        let refs = &mut row["resolution"]["faces"][0]["blocks"][1]["refs"];
        refs[0]["ref"] = json!({"ref":"event-player","role":"enters"});
    })));
    assert!(composition_rejects(&changed(DRAGON, |row| {
        let refs = row["resolution"]["faces"][0]["blocks"][1]["refs"]
            .as_array_mut()
            .unwrap();
        refs.push(refs[0].clone());
    })));
    // A printed clause the AST does not carry.
    assert!(rejected(&changed(SEAL, |row| {
        let text = format!(
            "{} Draw a card.",
            row["source"]["oracle_text"].as_str().unwrap()
        );
        row["source"]["oracle_text"] = json!(text);
        row["faces"][0]["result"]["lines"][0]["text"] = json!(text);
    })));
    // A changed cost term that reference binding does not cover.
    assert!(rejected(&changed(STRIP, |row| {
        row["faces"][0]["result"]["abilities"][1]["costs"][0] =
            json!({"cost":"mana","symbols":["1"]});
    })));
    // Variable costs have no block table row.
    assert!(rejected(
        &changed(PIE, |row| row["source"]["mana_cost"] = json!("{X}{B}"))
    ));
    // An intervening condition on a table event.
    assert!(rejected(&changed(MARSH, |row| {
        row["faces"][0]["result"]["abilities"][1]["condition"] = json!({"condition":"cards-in-hand","who":{"player":"you"},
                "comparison":{"op":"eq","value":{"amount":"fixed","value":0}}});
    })));
}

#[test]
fn timing_and_step_tables_lower_sorcery_speed_and_combat_restrictions() {
    // CR 602.5d: a sorcery-speed activation announces at main-phase timing.
    let restrict = |row: &mut serde_json::Value| {
        row["faces"][0]["result"]["abilities"][0]["restriction"] = json!({"restriction":"sorcery"});
    };
    // The restriction is one more printed sentence; without it the row's
    // text and AST disagree.
    assert!(!matches!(
        compile(R4, &changed(SEAL, restrict)),
        CompileResult::Lowered { .. }
    ));
    let sorcery = changed(SEAL, |row| {
        restrict(row);
        let printed = row["source"]["oracle_text"].as_str().unwrap().to_owned();
        let text = format!("{printed} Activate only as a sorcery.");
        row["source"]["oracle_text"] = json!(text);
        row["faces"][0]["result"]["lines"][0]["text"] = json!(text);
        // The certificate the parser emits for the added clause.
        let spans = &mut row["resolution"]["faces"][0]["blocks"][0]["spans"]["certificate"];
        spans["bytes"] = json!(text.len());
        spans["segments"].as_array_mut().unwrap().push(
            json!({"start":printed.len() + 1,"end":text.len(),"fields":["/costs","/restriction"]}),
        );
    });
    let CompileResult::Lowered { definition } = compile(R4, &sorcery) else {
        panic!("timing row must lower")
    };
    assert!(definition.composed);
    assert_eq!(
        definition.activated[0].program.timing,
        SpellTiming::MainPhase
    );
    assert_eq!(
        composed(SEAL).activated[0].program.timing,
        SpellTiming::Instant
    );
    assert_eq!(
        timing(Some(json!({"restriction":"sorcery"}))).unwrap().0,
        SpellTiming::MainPhase
    );
    assert_eq!(timing(None).unwrap().0, SpellTiming::Instant);
    assert!(timing(Some(json!({"restriction":"sorcery","turn":"your"}))).is_err());
    // CR 509.1b: one announced creature, restricted until cleanup.
    let step = json!({"effect":"cant","action":"block","duration":{"duration":"this-turn"},
        "what":{"ref":"target","filter":{"types":["creature"]},
            "count":{"amount":"fixed","value":1},"selectionId":"line:0:3"}});
    assert!(steps::claims(&step));
    let mut program = SpellProgram::instructions(SpellTiming::Instant);
    let instruction =
        steps::lower(&step, &mut program, &mut BTreeMap::new(), Context::Spell, 0).unwrap();
    assert_eq!(
        instruction,
        Instruction::GrantCreatureRestriction {
            target_slot: 0,
            restriction: CreatureRestriction::CantBlock,
            source_block: 0
        }
    );
    assert_eq!(program.targets[0].domain, TargetDomain::BattlefieldCreature);
    for unclaimed in [
        json!({"effect":"cant","action":"attack","duration":{"duration":"this-turn"},"what":step["what"]}),
        json!({"effect":"cant","action":"block","duration":{"duration":"end-of-turn"},"what":step["what"]}),
        json!({"effect":"cant","action":"block","duration":{"duration":"this-turn"},"what":step["what"],"extra":1}),
        json!({"effect":"cant","action":"block","duration":{"duration":"this-turn"},
            "what":{"ref":"target","filter":{"types":["creature"],"control":"opponent"},
                "count":{"amount":"fixed","value":1},"selectionId":"line:0:3"}}),
    ] {
        assert!(!steps::claims(&unclaimed), "{unclaimed}");
    }
}

/// Archive R4 negative mutations: each changes the resolution, the printed
/// text or a span without the other side agreeing, and each rejects with the
/// check that caught it rather than lowering something else.
#[test]
fn r4_resolution_mutations_reject() {
    const CHALICE: &str =
        include_str!("../../../../tests/fixtures/phase1-r4/compose-marble-chalice.json");
    const SAGA: &str =
        include_str!("../../../../tests/fixtures/phase1-r4/burn-burn-tree-and-fern.json");
    let archive = accepted_manifest(R4).unwrap();
    let path = |raw: &str| lower(archive, raw).map(|_| ()).unwrap_err().path;
    assert!(lower(archive, CHALICE).is_ok());
    // A swapped controller: "you" in an activated ability is its controller.
    let swapped = changed(CHALICE, |row| {
        let refs = &mut row["resolution"]["faces"][0]["blocks"][0]["refs"];
        let you = refs
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["ref"]["ref"] == "controller")
            .unwrap();
        you["ref"]["role"] = json!("spell-controller");
    });
    assert_eq!(path(&swapped), "resolution");
    // A truncated activation restriction: the AST keeps the field, the text
    // no longer prints it.
    let truncated = changed(SEAL, |row| {
        row["faces"][0]["result"]["abilities"][0]["restriction"] = json!({"restriction":"sorcery"});
    });
    assert_eq!(path(&truncated), "resolution.spans");
    // A dropped mid-sentence word: the certificate no longer tiles the text.
    let dropped = changed(STRIP, |row| {
        let drop = |text: &serde_json::Value| {
            json!(
                text.as_str()
                    .unwrap()
                    .replace("Destroy target land", "Destroy land")
            )
        };
        row["source"]["oracle_text"] = drop(&row["source"]["oracle_text"]);
        let line = &mut row["faces"][0]["result"]["lines"][1]["text"];
        *line = drop(line);
    });
    assert!(lower(archive, &dropped).is_err());
    let dropped_block = changed(STRIP, |row| {
        let text = &mut row["faces"][0]["result"]["lines"][1]["text"];
        *text = json!(
            text.as_str()
                .unwrap()
                .replace("Destroy target land", "Destroy land")
        );
    });
    assert!(lower(archive, &dropped_block).is_err());
    // A retargeted step entry or a dangling reference path.
    let retargeted = changed(STRIP, |row| {
        row["resolution"]["faces"][0]["blocks"][1]["steps"][0]["path"] =
            json!("/effects/0/steps/1");
    });
    assert_eq!(path(&retargeted), "resolution");
    // A span shifted into a multi-byte character ("—" is three bytes).
    let read = |resolution: String| {
        let sha = crate::sha256(resolution.as_bytes());
        resolution::Resolution::read(
            Some(&*RawValue::from_string(resolution).unwrap()),
            Some(&sha),
        )
        .unwrap()
    };
    let row: serde_json::Value = serde_json::from_str(SAGA).unwrap();
    let resolution = read(row["resolution"].to_string());
    let (text, ability) = (
        row["faces"][0]["result"]["lines"][1]["text"]
            .as_str()
            .unwrap(),
        &row["faces"][0]["result"]["abilities"][1],
    );
    assert!(resolution.spans(1, text, ability).is_ok());
    let mut shifted = row["resolution"].clone();
    let dash = text.find('—').unwrap();
    shifted["faces"][0]["blocks"][1]["spans"]["certificate"]["segments"][0]["end"] =
        json!(dash + 1);
    let shifted = read(shifted.to_string());
    assert!(shifted.spans(1, text, ability).is_err());
}

const BONES: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/compose-read-the-bones.json");
const BAUBLE: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/compose-wayfarers-bauble.json");
const GALVANIC: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/compose-galvanic-blast.json");

/// Composition is the only pipeline: a record without an Archive R4
/// resolution is rejected before any block is read, never lowered by a
/// diagnostic-graph fallback.
#[test]
fn records_without_an_r4_resolution_reject() {
    let pre_r4 = include_str!("../../../../tests/fixtures/grizzly-bears.json");
    assert!(matches!(
        compile(crate::fixtures::MANIFEST, pre_r4),
        CompileResult::Unsupported { ref path, .. } if path == "resolution"
    ));
}

/// CR 121.2: "draw two cards" is two single draws, the same canonical form
/// for every block kind; Read the Bones keeps its printed order around them.
#[test]
fn fixed_draws_lower_to_ordered_single_draws() {
    let definition = composed(BONES);
    let spell = definition.spell.as_ref().unwrap();
    let draw = Instruction::draw(kir::PlayerExpr::you(kir::Context::Spell), 1.into(), 0);
    assert_eq!(spell.instructions.len(), 4);
    assert!(matches!(spell.instructions[0], Instruction::Scry { .. }));
    assert_eq!(spell.instructions[1..3], [draw.clone(), draw]);
    assert!(
        definition
            .capabilities
            .contains(&Capability::ControllerDrawV1)
    );
}

/// "Search ... for a basic land card, put that card onto the battlefield
/// tapped, then shuffle" is the one-card tapped basic land search, whatever
/// cost pays for it.
#[test]
fn paid_basic_land_search_is_the_tapped_search() {
    let definition = composed(BAUBLE);
    assert_eq!(
        definition.activated[0].program.instructions,
        [Instruction::SearchBasicLands {
            maximum: 1,
            source_block: 0,
        }]
    );
}

/// The parser keeps an ability-word rider ("Metalcraft — ... instead") in
/// the block it modifies. Every printed line is accounted for, but the block
/// mirror cannot represent that grouping: reject as unsupported, and keep
/// `invalid` for text the parse does not account for.
#[test]
fn rider_blocks_reject_without_claiming_unaccounted_text() {
    assert!(matches!(
        compile(R4, GALVANIC),
        CompileResult::Unsupported { ref path, .. } if path == "lines"
    ));
    let truncated = changed(GALVANIC, |row| {
        let text = row["source"]["oracle_text"].as_str().unwrap().to_owned();
        row["source"]["oracle_text"] = format!("{text}\nDraw a card.").into();
    });
    assert!(matches!(
        compile(R4, &truncated),
        CompileResult::Invalid { ref path, .. } if path == "lines"
    ));
}

/// CR 608.2c, 601.2c: the lowerer binds a pronoun to the nearest earlier
/// producer of its kind and a printed target to its announced slot. An
/// archived edge that names another producer, role or slot, or a deleted
/// edge, rejects instead of being read by shape.
#[test]
fn contradictory_or_missing_r4_edges_reject() {
    assert!(matches!(compile(R4, BAUBLE), CompileResult::Lowered { .. }));
    let refs = |row: &mut serde_json::Value| -> serde_json::Value {
        row["resolution"]["faces"][0]["blocks"][0]["refs"].clone()
    };
    let edge = |change: &dyn Fn(&mut serde_json::Value)| {
        changed(BAUBLE, |row| {
            let mut entries = refs(row);
            for entry in entries.as_array_mut().unwrap() {
                if entry["path"] == "/effects/0/steps/1/what" {
                    change(entry);
                }
            }
            row["resolution"]["faces"][0]["blocks"][0]["refs"] = entries;
        })
    };
    let rejected = |raw: &str| matches!(compile(R4, raw), CompileResult::Unsupported { ref path, .. } if path == "resolution");
    // A later producer, the wrong role and a step that produces nothing.
    assert!(rejected(&edge(
        &|e| e["ref"]["step"] = json!("/effects/0/steps/2/for")
    )));
    assert!(rejected(&edge(&|e| e["ref"]["role"] = json!("objects"))));
    assert!(rejected(&edge(
        &|e| e["ref"]["step"] = json!("/effects/0/steps/0")
    )));
    // A deleted edge leaves the operand unresolved.
    let deleted = changed(BAUBLE, |row| {
        let mut entries = refs(row);
        entries
            .as_array_mut()
            .unwrap()
            .retain(|e| e["path"] != "/effects/0/steps/1/what");
        row["resolution"]["faces"][0]["blocks"][0]["refs"] = entries;
    });
    assert!(rejected(&deleted));
    // A printed target the resolution assigns another slot.
    assert!(matches!(compile(R4, SEAL), CompileResult::Lowered { .. }));
    let slot = changed(SEAL, |row| {
        row["resolution"]["faces"][0]["blocks"][0]["refs"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|e| e["ref"]["ref"] == "target")
            .for_each(|e| e["ref"]["slot"] = json!(1));
    });
    assert!(rejected(&slot));
}
