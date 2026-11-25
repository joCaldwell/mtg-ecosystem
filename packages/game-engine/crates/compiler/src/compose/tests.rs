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
                pay_life: None,
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

/// The failure lowering reports once binding checks are deferred.
fn lowering(found: &Blockers) -> Option<&Failure> {
    found.blocks.first().map(|(_, f)| f).or(found.card.as_ref())
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
            found.bindings.first().map(|f| f.path.clone()),
            lowering(found).map(|f| f.path.clone()),
            found.blocks.first().map(|(block, _)| *block),
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
    assert_eq!(lowering(&found).unwrap().reason, failure.reason);
    assert_eq!(found.blocks.len(), 1);
    // Every failing block is named, not only the first: lowering stops at
    // block 0, the report goes on to block 1.
    let two = changed(STRIP, |row| {
        for block in [0, 1] {
            row["faces"][0]["result"]["abilities"][block]["abilityWord"] = json!("Unprinted");
        }
    });
    assert!(lower(archive, &two).is_err());
    let found = blockers(archive, &two);
    assert_eq!(
        found
            .blocks
            .iter()
            .map(|(block, f)| (*block, f.path.as_str()))
            .collect::<Vec<_>>(),
        [(0, "resolution.spans"), (1, "resolution.spans")]
    );
    assert!(found.card.is_none());
    // Every step the resolution leaves unaccounted is its own binding
    // blocker; lowering still rejects at the first.
    let unaccounted = changed(STRIP, |row| {
        for (block, effect) in [(0, "add-mana"), (1, "destroy")] {
            row["resolution"]["faces"][0]["blocks"][block]["steps"] = json!([
                {"path":"/effects/0/steps/0","astEffect":effect,"unaccounted":true}]);
        }
    });
    assert_eq!(
        lower(archive, &unaccounted).unwrap_err().reason,
        "step:add-mana"
    );
    assert_eq!(
        blockers(archive, &unaccounted)
            .bindings
            .iter()
            .map(|f| f.reason.as_str())
            .collect::<Vec<_>>(),
        ["step:add-mana", "step:destroy"]
    );
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
fn timing_table_lowers_printed_sorcery_speed() {
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
    let pre_r4 = changed(STRIP, |row| {
        row.as_object_mut().unwrap().remove("resolution");
        row.as_object_mut().unwrap().remove("resolutionSha256");
    });
    assert!(matches!(
        compile(R4, &pre_r4),
        CompileResult::Unsupported { ref path, .. } if path == "resolution"
    ));
}

/// CR 701.22a, 701.23a, 701.24a: Read the Bones scries and Wayfarer's
/// Bauble searches and shuffles through `Executes`-only library cells.
#[test]
fn library_procedures_lower_to_executes_only_cells() {
    use kir::{Cell, Tiers, definition_cells, definition_tiers};
    for (raw, cells) in [
        (BONES, vec![Cell::ScryController]),
        (
            BAUBLE,
            vec![Cell::SearchController, Cell::ShuffleController],
        ),
    ] {
        let CompileResult::Lowered { definition } = compile(R4, raw) else {
            panic!("library procedure did not lower");
        };
        assert_eq!(definition_tiers(&definition).unwrap(), Tiers::EXECUTES);
        let occupied = definition_cells(&definition).unwrap();
        assert!(cells.iter().all(|cell| occupied.contains(cell)));
    }
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
    // A deleted scalar owner edge ("your library") is as unresolved.
    let owner = changed(BAUBLE, |row| {
        let mut entries = refs(row);
        entries
            .as_array_mut()
            .unwrap()
            .retain(|e| e["path"] != "/effects/0/steps/0/zones/of/0/owner");
        row["resolution"]["faces"][0]["blocks"][0]["refs"] = entries;
    });
    assert!(rejected(&owner));
    // "That card" as the remainder of its own move, and as an evaluated
    // selection with no producer.
    assert!(rejected(&edge(&|e| {
        e["ref"] = json!({"ref":"step-result","step":"/effects/0/steps/1","role":"remainder"})
    })));
    assert!(rejected(&edge(&|e| {
        e.as_object_mut().unwrap().remove("ref");
        e["evaluated"] = json!("selection");
    })));
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

#[test]
fn explicit_nulls_reject_under_their_field_before_any_decoder() {
    let code = |raw: &str| match compile(R4, raw) {
        CompileResult::Unsupported { path, reason } => {
            crate::coverage::reason_code(&path, &reason, &serde_json::Value::Null)
        }
        other => panic!("{other:?}"),
    };
    const UNSUMMON: &str = include_str!("../../../../tests/fixtures/phase1-r4/unsummon.json");
    for witness in [MARSH, UNSUMMON] {
        assert!(matches!(
            compile(R4, witness),
            CompileResult::Lowered { .. }
        ));
    }
    // A null object member: the player mirror would drop the selection.
    let marsh = changed(MARSH, |row| {
        row["faces"][0]["result"]["abilities"][1]["effects"][0]["steps"][0]["who"]["selectionId"] =
            serde_json::Value::Null;
    });
    assert_eq!(code(&marsh), "operand:ast:null:selectionId");
    // A null array element, named by its array's field.
    let unsummon = changed(UNSUMMON, |row| {
        row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]["types"] =
            json!(["creature", null]);
    });
    assert_eq!(code(&unsummon), "operand:ast:null:types");
    // A null abilities list is named by its own field.
    let empty = crate::tests::changed_source(UNSUMMON, |row| {
        row["faces"][0]["result"]["abilities"] = serde_json::Value::Null;
    });
    assert_eq!(code(&empty), "operand:ast:null:abilities");
    // Null target-filter qualifiers reject the same way.
    for field in ["control", "other"] {
        let qualified = changed(UNSUMMON, |row| {
            row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]
                [field] = serde_json::Value::Null;
        });
        assert_eq!(code(&qualified), format!("operand:ast:null:{field}"));
    }
}

// Wave W5 control (CR 118.12, 608.2c-d): frozen R4 witnesses.
const RHYSTIC_STUDY: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/53236dd7-845a-444c-96d5-f41ed7325d8f.json");
const WOODEN_SPHERE: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/0bd8917c-bec4-4603-bc3f-8e0c2afae56a.json");
const SOULS_ATTENDANT: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/045a9d3d-c20d-427f-a77b-0bffc6f55526.json");
const ROADSIDE_RELIQUARY: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/2fb13687-0518-4ba0-a5ae-dd609464b026.json");
const AESI: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/6511f317-bd38-46d0-b800-7125a3f420da.json");
const SCHOLAR_OF_STARS: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/0753aee4-33db-48c5-9854-16a9d91535b2.json");
const MIRE_KAVU: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/0bc58096-7925-4870-bb95-58cce51ea8e7.json");
const TEMPORAL_MACHINATIONS: &str =
    include_str!("../../../../tests/fixtures/phase1-r4/f0adab66-c3fe-4989-b7c8-ab7bdc7f0fd1.json");

/// A newly opened control surface earns `Executes` and nothing above it.
fn executes_only(definition: &Definition) {
    let tiers = kir::definition_tiers(definition).unwrap();
    assert!(tiers.admits(kir::ExecutionTier::Executes));
    for tier in [
        kir::ExecutionTier::HeadlessQualified,
        kir::ExecutionTier::Search,
        kir::ExecutionTier::Hosting,
    ] {
        assert!(!tiers.admits(tier), "{} leaks {tier:?}", definition.name);
    }
    assert!(
        kir::definition_cells(definition)
            .unwrap()
            .contains(&kir::Cell::Control)
    );
}

fn only_node(program: &SpellProgram) -> &kir::Node {
    let [Instruction::Kernel { node, .. }] = program.instructions.as_slice() else {
        panic!("one control instruction: {:?}", program.instructions);
    };
    node
}

#[test]
fn printed_may_unless_and_if_lower_to_kernel_control_nodes() {
    use kir::{Node, Payment};
    let drawn = |node: &Node| {
        matches!(node, Node::Do(Verb::Draw { player, amount })
            if player.is_you() && amount.as_fixed() == Some(&Quantity::from(1)))
    };
    // "You may draw a card unless that player pays {1}": the caster's payment
    // decides first, then the controller's choice (CR 118.12a, 608.2d).
    let study = composed(RHYSTIC_STUDY);
    let Node::UnlessPays {
        payer,
        payment: Payment::Generic { amount },
        node,
    } = only_node(&study.triggered[0].program)
    else {
        panic!("{:?}", study.triggered[0].program);
    };
    assert_eq!(*payer, PlayerExpr::event(kir::EventPlayerRole::Caster));
    assert_eq!(amount.as_fixed(), Some(&Quantity::from(1)));
    assert!(matches!(&**node, Node::Optional { chooser, node } if chooser.is_you() && drawn(node)));
    // "You may pay {1}. If you do, you gain 1 life."
    let sphere = composed(WOODEN_SPHERE);
    assert!(matches!(
        only_node(&sphere.triggered[0].program),
        Node::IfPays { payer, payment: Payment::Generic { amount }, node }
            if payer.is_you()
                && amount.as_fixed() == Some(&Quantity::from(1))
                && matches!(&**node, Node::Do(Verb::GainLife { .. }))
    ));
    // "You may gain 1 life."
    let attendant = composed(SOULS_ATTENDANT);
    assert!(matches!(
        only_node(&attendant.triggered[0].program),
        Node::Optional { chooser, node }
            if chooser.is_you() && matches!(&**node, Node::Do(Verb::GainLife { .. }))
    ));
    // "Return target creature ... If you control an artifact, draw a card."
    // The condition is read on resolution (CR 608.2c); the return is plain.
    let machinations = composed(TEMPORAL_MACHINATIONS);
    let spell = machinations.spell.as_ref().unwrap();
    let [
        Instruction::Kernel { node: first, .. },
        Instruction::Kernel { node: second, .. },
    ] = spell.instructions.as_slice()
    else {
        panic!("{spell:?}");
    };
    assert!(matches!(first, Node::Do(Verb::Move { .. })));
    let Node::If {
        condition:
            kir::Cond::Exists {
                set: SetExpr::Matching { zone, filter },
            },
        then,
        otherwise: None,
    } = second
    else {
        panic!("{second:?}");
    };
    assert_eq!(*zone, kir::ZoneExpr::Battlefield);
    assert!(filter.0.contains(&kir::Atom::Type {
        kind: CardKind::Artifact
    }));
    assert!(filter.0.iter().any(|atom| matches!(atom,
        kir::Atom::ControlledBy { player } if player.is_you())));
    assert!(drawn(then));
    for definition in [&study, &sphere, &attendant, &machinations] {
        executes_only(definition);
    }
}

#[test]
fn control_without_its_printed_clause_or_with_an_unread_condition_rejects() {
    let code = |raw: &str| match compile(R4, raw) {
        CompileResult::Unsupported { path, reason } => {
            crate::coverage::reason_code(&path, &reason, &serde_json::Value::Null)
        }
        other => panic!("expected a rejection, got {other:?}"),
    };
    fn ast(row: &mut serde_json::Value) -> &mut serde_json::Value {
        &mut row["faces"][0]["result"]["abilities"][0]["effects"]
    }
    // A printed "may" whose AST choice was removed is mandatory against its text.
    assert_eq!(
        code(&changed(SOULS_ATTENDANT, |row| {
            ast(row)[0]["steps"][0]
                .as_object_mut()
                .unwrap()
                .remove("optional");
        })),
        "compose.effects:printed_choice_limit_or_condition_disagrees_with_the_ast"
    );
    // A payment no "if you do" reads is no control construct.
    assert!(matches!(
        compile(
            R4,
            &changed(WOODEN_SPHERE, |row| {
                ast(row)[0]["steps"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("optional");
            })
        ),
        CompileResult::Unsupported { .. }
    ));
    // "If that player pays" has no payment row; it is not "unless".
    assert_eq!(
        code(&changed(RHYSTIC_STUDY, |row| {
            ast(row)[0]["conditionKind"] = json!("if");
        })),
        "compose.control:condition_pays"
    );
    // A condition outside the board rows rejects instead of guessing.
    assert_eq!(
        code(&changed(TEMPORAL_MACHINATIONS, |row| {
            ast(row)[1]["condition"]["what"]["count"]["value"] = json!(2);
        })),
        "compose.control:control_shape"
    );
}

/// Rewrite one reference entry of block `block` in the R4 resolution.
fn reresolved(raw: &str, block: usize, path: &str, resolved: serde_json::Value) -> String {
    changed(raw, |row| {
        let refs = row["resolution"]["faces"][0]["blocks"][block]["refs"]
            .as_array_mut()
            .unwrap();
        let entry = refs.iter_mut().find(|r| r["path"] == path).unwrap();
        entry["ref"] = resolved;
    })
}

fn rejection(raw: &str) -> String {
    match compile(R4, raw) {
        CompileResult::Unsupported { path, reason } => {
            crate::coverage::reason_code(&path, &reason, &serde_json::Value::Null)
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

const DISAGREES: &str = "compose.effects:printed_choice_limit_or_condition_disagrees_with_the_ast";

#[test]
fn printed_choices_match_their_carriers_in_number_and_polarity() {
    fn effects(row: &mut serde_json::Value, block: usize) -> &mut serde_json::Value {
        &mut row["faces"][0]["result"]["abilities"][block]["effects"]
    }
    // Both conditions lower; dropping one leaves a printed "if" unconsumed.
    let reliquary = composed(ROADSIDE_RELIQUARY);
    assert!(matches!(
        reliquary.activated[0].program.instructions.as_slice(),
        [
            Instruction::Kernel {
                node: kir::Node::If { .. },
                ..
            },
            Instruction::Kernel {
                node: kir::Node::If { .. },
                ..
            },
        ]
    ));
    let dropped = changed(ROADSIDE_RELIQUARY, |row| {
        let first = effects(row, 1)[0].as_object_mut().unwrap();
        first.remove("condition");
        first.remove("conditionKind");
        first.remove("conditionPosition");
        row["faces"][0]["result"]["lines"][1]["ability"] =
            row["faces"][0]["result"]["abilities"][1].clone();
        row["resolution"]["faces"][0]["blocks"][1]["refs"]
            .as_array_mut()
            .unwrap()
            .retain(|r| {
                !r["path"]
                    .as_str()
                    .unwrap()
                    .starts_with("/effects/0/condition")
            });
    });
    assert_eq!(rejection(&dropped), DISAGREES);
    // "If" and "unless" cannot stand in for each other.
    assert_eq!(
        rejection(&changed(TEMPORAL_MACHINATIONS, |row| {
            effects(row, 0)[1]["conditionKind"] = json!("unless");
        })),
        DISAGREES
    );
    // A condition moved to the sentence that does not print it.
    let moved = changed(TEMPORAL_MACHINATIONS, |row| {
        let effects = effects(row, 0);
        let second = effects[1].as_object_mut().unwrap();
        let (condition, kind) = (
            second.remove("condition").unwrap(),
            second.remove("conditionKind").unwrap(),
        );
        second.remove("conditionPosition");
        effects[0]["condition"] = condition;
        effects[0]["conditionKind"] = kind;
        row["faces"][0]["result"]["lines"][0]["ability"] =
            row["faces"][0]["result"]["abilities"][0].clone();
        for entry in row["resolution"]["faces"][0]["blocks"][0]["refs"]
            .as_array_mut()
            .unwrap()
        {
            let path = entry["path"].as_str().unwrap().to_owned();
            if let Some(rest) = path.strip_prefix("/effects/1/condition") {
                entry["path"] = json!(format!("/effects/0/condition{rest}"));
            }
        }
    });
    assert_eq!(rejection(&moved), DISAGREES);
    // "If you do" cannot lower as "if you don't" (CR 118.12).
    let inverted = changed(WOODEN_SPHERE, |row| {
        effects(row, 0)[1]["condition"] = json!({"condition": "you-dont"});
        row["faces"][0]["result"]["lines"][0]["ability"] =
            row["faces"][0]["result"]["abilities"][0].clone();
    });
    assert_eq!(rejection(&inverted), DISAGREES);
    // A static block is checked too. Aesi's printed "may" is its land-play
    // permission; Mire Kavu's "as long as" is its condition, and without it
    // the static would lower as an unconditional +1/+1.
    composed(AESI);
    composed(MIRE_KAVU);
    let unconditional = changed(MIRE_KAVU, |row| {
        let ability = &mut row["faces"][0]["result"]["abilities"][0];
        ability["effect"]
            .as_object_mut()
            .unwrap()
            .remove("condition");
        row["faces"][0]["result"]["lines"][0]["ability"] = ability.clone();
        row["resolution"]["faces"][0]["blocks"][0]["refs"]
            .as_array_mut()
            .unwrap()
            .retain(|r| !r["path"].as_str().unwrap().starts_with("/effect/condition"));
    });
    assert_eq!(rejection(&unconditional), DISAGREES);
}

#[test]
fn payers_and_choosers_bind_exactly_the_printed_player() {
    // The cost's implicit payer contradicts the explicit payer.
    assert_eq!(
        rejection(&reresolved(
            WOODEN_SPHERE,
            0,
            "/effects/0/steps/0/costs/0/payer",
            json!({"ref": "event-player", "role": "caster"}),
        )),
        "compose.control:payer_disagrees"
    );
    assert_eq!(
        rejection(&reresolved(
            RHYSTIC_STUDY,
            0,
            "/effects/0/condition/cost/0/payer",
            json!({"ref": "controller", "role": "ability-controller"}),
        )),
        "compose.control:payer_disagrees"
    );
    // A chooser other than the controller (CR 800.4g reassignment is not
    // performed by the continuation).
    let other = changed(RHYSTIC_STUDY, |row| {
        row["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["decidedBy"] =
            json!({"player": "that-player"});
        row["faces"][0]["result"]["lines"][0]["ability"] =
            row["faces"][0]["result"]["abilities"][0].clone();
        row["resolution"]["faces"][0]["blocks"][0]["refs"]
            .as_array_mut()
            .unwrap()
            .push(json!({"path": "/effects/0/steps/0/decidedBy",
                         "ref": {"ref": "event-player", "role": "caster"}}));
    });
    assert_eq!(rejection(&other), "compose.control:optional_chooser");
}

/// CR 603.4: R4 certifies "When this creature enters, if you control an
/// artifact," as one header span naming `/trigger` and `/condition`. The
/// printed guards accept it, and the block reaches the trigger lowerer's own
/// intervening-if decision instead of a resolution-only `If`.
#[test]
fn a_certified_trigger_header_with_its_condition_reaches_the_trigger_decision() {
    assert_eq!(
        rejection(SCHOLAR_OF_STARS),
        "abilities.condition:unsupported_complete_intervening_if_condition"
    );
    // Without its block condition the same printed header is a spans defect,
    // never a sentence condition.
    let dropped = changed(SCHOLAR_OF_STARS, |row| {
        let ability = &mut row["faces"][0]["result"]["abilities"][0];
        let condition = ability
            .as_object_mut()
            .unwrap()
            .remove("condition")
            .unwrap();
        ability["effects"][0]["condition"] = condition;
        ability["effects"][0]["conditionKind"] = json!("if");
        row["faces"][0]["result"]["lines"][0]["ability"] = ability.clone();
    });
    assert!(!matches!(
        compile(R4, &dropped),
        CompileResult::Lowered { .. }
    ));
}
