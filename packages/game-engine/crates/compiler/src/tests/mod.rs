//! Compiler unit tests: exact frozen rows and targeted shape/rejection cases.
use super::*;

mod ability_sequence_tests;
mod absent_mana_cost_tests;
mod activated_permanent_return_tests;
mod activated_permanent_tap_tests;
mod activated_spell_counter_tests;
mod activation_source_tests;
mod announced_x_effects_tests;
mod artifact_sacrifice_source_tests;
mod aura_control_tests;
mod aura_return_binding_tests;
mod board_filter_tests;
mod collar_source_tests;
mod composition_wave_a_tests;
mod composition_wave_c_tests;
mod compound_indestructible_family_tests;
mod compound_indestructible_tests;
mod controlled_temporary_creature_keyword_tests;
mod controller_counted_draw_tests;
mod controller_scry_tests;
mod controller_surveil_tests;
mod counter_null_filter_tests;
mod counter_placement_tests;
mod counter_spell_controller_mill_tests;
mod counter_target_reuse_tests;
mod creature_set_keyword_tests;
mod cycling_tests;
mod d2b_tests;
mod d2c_tests;
mod d2cb_tests;
mod d2cc_tests;
mod d2cd_tests;
mod d2ce_tests;
mod draw_discard_tests;
mod equipped_indestructible_tests;
mod escort_set_tests;
mod fixed_mill_tests;
mod hand_return_tests;
mod indestructible_tests;
mod library_order_tests;
mod library_selection_tests;
mod modal_binding_tests;
mod null_flag_tests;
mod optional_operand_tests;
mod permanent_owner_hand_tests;
mod permanent_set_keyword_tests;
mod phase1_sequences_tests;
mod r4_binding_tests;
mod ramos_m1_tests;
mod ramos_m3_tests;
mod ramos_m3c_tests;
mod ramos_m4a_tests;
mod ramos_m4b_tests;
mod ramos_m4c_tests;
mod ramos_m4d_tests;
mod ramos_m4e_tests;
mod source_label_tests;
mod tap_source_destroy_tests;
mod tap_source_untap_tests;
mod tap_tests;
mod tapped_target_tests;
mod targeted_discard_tests;
mod temporary_combat_keyword_tests;
mod temporary_creature_keyword_tests;
mod temporary_deathtouch_tests;
mod temporary_double_strike_tests;
mod temporary_flying_tests;
mod temporary_indestructible_tests;
mod temporary_permanent_keyword_tests;
mod temporary_reach_tests;
mod temporary_vigilance_tests;
mod type_filter_null_tests;
mod untap_tests;

#[test]
fn fixed_opponent_damage_rejects_unbound_target_substitution() {
    let changed = changed_source(
        include_str!("../../../../tests/fixtures/lightning-bolt.json"),
        |r| {
            let result = &mut r["faces"][0]["result"];
            result["abilities"][0]["effects"][0]["steps"][0]["to"] = serde_json::json!({
                "to":"player", "ref":{"player":"target-opponent","selectionId":"line:0:6"}
            });
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        },
    );
    assert!(matches!(
        compile(fixtures::MANIFEST, &changed),
        CompileResult::Unsupported { .. }
    ));
}

#[test]
fn bounded_token_types_preserve_single_type_bytes() {
    let mut source: SourceToken = serde_json::from_value(serde_json::json!({
        "types":["artifact","creature"], "supertypes":[], "subtypes":["Latticekin"],
        "colors":[], "keywords":[], "power":{"amount":"fixed","value":3},
        "toughness":{"amount":"fixed","value":7}
    }))
    .unwrap();
    let token = lower_creature_token(&source).unwrap();
    assert!(token.characteristics().has_type(CardKind::Artifact));
    source.types.reverse();
    assert_eq!(lower_creature_token(&source).unwrap(), token);
    source.types.pop();
    let single = lower_creature_token(&source).unwrap();
    let bytes = serde_json::to_value(&single).unwrap();
    assert!(bytes.get("additional_types").is_none());
    assert_eq!(
        serde_json::from_value::<CreatureToken>(bytes).unwrap(),
        single
    );
    assert!(!single.characteristics().has_type(CardKind::Artifact));
    source.types.push("land".into());
    assert!(lower_creature_token(&source).is_err());
}

#[test]
fn strict_mirror_reports_specific_nodes_keywords_and_fields() {
    for (raw, expected) in [
        (
            r#"[{"kind":"future-ability","payload":{}}]"#,
            "kind:future-ability",
        ),
        (
            // Combat damage is supported; this broader player event is not.
            r#"[{"kind":"triggered","trigger":{"trigger":"player-dealt-damage"},"effects":[]}]"#,
            "trigger:player-dealt-damage",
        ),
        (
            r#"[{"kind":"spell","effects":[{"steps":[{"effect":"future-effect"}]}]}]"#,
            "effect:future-effect",
        ),
        (
            r#"[{"kind":"keywords","keywords":[{"keyword":"ward","cost":[]}]}]"#,
            "keyword:ward",
        ),
        (
            r#"[{"kind":"keywords","keywords":[{"keyword":"flying","extra":true}]}]"#,
            "keyword:flying:field:extra",
        ),
    ] {
        let failure = read_abilities(raw).unwrap_err();
        assert!(failure.unsupported);
        assert_eq!(failure.reason, expected);
    }
    let failure =
        read_abilities(r#"[{"kind":"spell","effects":[],"unknownClause":true}]"#).unwrap_err();
    assert!(failure.unsupported && failure.reason.contains("unknownClause"));
    let failure =
        read_abilities(r#"[{"kind":"keywords","keywords":[],"keywords":[]}]"#).unwrap_err();
    assert!(!failure.unsupported, "duplicates remain invalid");
    let raw = fixtures::COMBAT_KEYWORD_RECORDS
        .iter()
        .find(|(name, _)| *name == "youthful-knight")
        .unwrap()
        .1;
    let alias = changed_source(raw, |r| {
        r["faces"][0]["result"]["abilities"][0]["keywords"][0]["keyword"] = "first-strike".into();
        r["faces"][0]["result"]["lines"][0]["ability"] =
            r["faces"][0]["result"]["abilities"][0].clone();
    });
    let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &alias) else {
        panic!("implemented keyword alias must lower");
    };
    assert_eq!(definition.keywords, [KeywordAbility::FirstStrike]);
}

#[test]
fn combined_types_preserve_all_types_and_reject_missing_runtime_types() {
    for line in [
        "Artifact Creature — Bear",
        "Legendary Artifact Creature — Bear",
        "Enchantment Creature — Bear",
        "Legendary Enchantment Creature — Bear",
        "Artifact Enchantment Creature — Bear",
        "Creature Artifact — Bear",
    ] {
        let raw = changed_record(1, |r| {
            r["source"]["type_line"] = line.into();
        });
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &raw) else {
            panic!("{line}");
        };
        definition.validate().unwrap();
        assert!(definition.characteristics.has_type(CardKind::Creature));
        assert_eq!(
            definition.characteristics.has_type(CardKind::Artifact),
            line.contains("Artifact")
        );
        assert_eq!(
            definition.characteristics.has_type(CardKind::Enchantment),
            line.contains("Enchantment")
        );
        assert_eq!(
            definition.characteristics.legendary,
            line.contains("Legendary")
        );
        let mut bad = *definition;
        bad.characteristics.additional_types.push(CardKind::Land);
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
    }
    let raw = fixtures::EFFECT_RECORDS
        .iter()
        .find(|(name, _)| *name == "furnace-of-rath")
        .unwrap()
        .1;
    let raw = changed_source(raw, |r| {
        r["source"]["type_line"] = "Legendary Enchantment".into();
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &raw),
        CompileResult::Lowered { .. }
    ));
    // Spell subtypes are retained metadata. Their printed abilities still
    // have to lower; Arcane/Lesson do not add an implicit runtime action.
    for subtype in ["Arcane", "Lesson"] {
        let raw = changed_record(3, |r| {
            r["source"]["type_line"] = format!("Instant — {subtype}").into();
        });
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &raw) else {
            panic!("inert spell subtype {subtype}");
        };
        assert_eq!(definition.characteristics.subtypes, [subtype]);
        definition.validate().unwrap();
    }
    for (line, reason) in [
        ("Planeswalker — Ada", "Planeswalker"),
        ("Battle — Siege", "Battle"),
        ("World Enchantment", "World"),
    ] {
        let raw = changed_record(1, |r| {
            r["source"]["type_line"] = line.into();
        });
        assert!(
            matches!(compile(fixtures::MANIFEST, &raw), CompileResult::Unsupported { reason: actual, .. } if actual == reason)
        );
    }
}

#[test]
fn multi_face_layout_is_unsupported_without_single_face_defaults() {
    let raw = changed_record(1, |r| {
        r["source"]["layout"] = "transform".into();
        r["source"]["card_faces"] = serde_json::json!([{}, {}]);
        r["source"].as_object_mut().unwrap().remove("oracle_text");
        r["source"].as_object_mut().unwrap().remove("mana_cost");
    });
    assert!(
        matches!(compile(fixtures::MANIFEST, &raw), CompileResult::Unsupported { reason, .. } if reason == "multi_face_layout:transform")
    );
    let mut bad: serde_json::Value = serde_json::from_str(&raw).unwrap();
    bad["sourceSha256"] = "0".repeat(64).into();
    assert!(matches!(
        compile(fixtures::MANIFEST, &bad.to_string()),
        CompileResult::Invalid { .. }
    ));
    let raw = changed_record(1, |r| {
        r["source"].as_object_mut().unwrap().remove("oracle_text");
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &raw),
        CompileResult::Invalid { .. }
    ));
}

#[test]
fn source_normalization_accounts_for_every_clause_and_literal_name() {
    assert_eq!(source_lines("When Aether Bear enters, put a counter on Aether Bear. Aether Bear's power is 2.\n(Outer (nested) reminder) Flying", "Aether Bear", "Creature").unwrap(), ["When ~ enters, put a counter on ~. ~'s power is 2.", "Flying"]);
    assert_eq!(
        source_lines(
            "When Ada enters, Ada, the Wise draws a card.",
            "Ada, the Wise",
            "Legendary Creature"
        )
        .unwrap(),
        ["When ~ enters, ~ draws a card."]
    );
    assert_eq!(
        source_lines("Fearsome fear Fear’s power is 1.", "Fear", "Creature").unwrap(),
        ["Fearsome fear ~'s power is 1."]
    );
    assert_eq!(
        source_lines(
            "Search for cards named Squadron Hawk.",
            "Squadron Hawk",
            "Creature"
        )
        .unwrap(),
        ["Search for cards named Squadron Hawk."]
    );
    assert_eq!(
        source_lines(
            "When Ada enters, target Ada planeswalker gains loyalty.",
            "Ada, the Wise",
            "Legendary Creature"
        )
        .unwrap(),
        ["When ~ enters, target Ada planeswalker gains loyalty."]
    );
    assert!(source_lines("Flying (unclosed", "Bear", "Creature").is_err());
    // The expected line is still byte-exact after normalization.
    let raw = changed_record(3, |r| {
        r["faces"][0]["result"]["lines"][0]["text"] = "When ~ enters, draw two cards.".into();
    });
    assert!(
        matches!(compile(fixtures::MANIFEST, &raw), CompileResult::Invalid { path, .. } if path == "lines")
    );
}

#[test]
fn wipes_complete_source_pairs() {
    let c = embedded_catalog("wipes").unwrap();
    assert_eq!(c.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        c,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-wipes-ir.json"
        ))
        .unwrap()
    );
    for (name, raw) in fixtures::WIPES_RECORDS {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("{name}")
        };
        assert!(c.definitions.contains(&definition), "{name}");
    }
    for raw in [
        include_str!("../../../../tests/fixtures/austere-command.json"),
        include_str!("../../../../tests/fixtures/fiery-confluence.json"),
        include_str!("../../../../tests/fixtures/bontus-last-reckoning.json"),
        include_str!("../../../../tests/fixtures/rolling-earthquake.json"),
    ] {
        assert!(!matches!(
            compile(fixtures::MANIFEST, raw),
            CompileResult::Lowered { .. }
        ));
    }
}
#[test]
fn sacrifice_complete_sources_and_whole_card_negatives() {
    let c = embedded_catalog("sacrifice").unwrap();
    assert_eq!(c.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        c,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-sacrifice-ir.json"
        ))
        .unwrap()
    );
    for (name, raw) in fixtures::SACRIFICE_RECORDS {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("{name}")
        };
        assert!(c.definitions.contains(&definition), "{name}");
    }
    for raw in [
        include_str!("../../../../tests/fixtures/ashnods-altar.json"),
        include_str!("../../../../tests/fixtures/morbid-opportunist.json"),
        include_str!("../../../../tests/fixtures/young-wolf.json"),
        include_str!("../../../../tests/fixtures/pitiless-plunderer.json"),
    ] {
        assert!(!matches!(
            compile(fixtures::MANIFEST, raw),
            CompileResult::Lowered { .. }
        ));
    }
}
#[test]
fn sacrifice_complete_filters_amounts_and_optional_clauses_cannot_disappear() {
    for (name, path, value) in [
        (
            "village-rites",
            "/costs/0/what/filter/control",
            serde_json::json!("opponent"),
        ),
        (
            "siege-gang-commander",
            "/costs/1/what/filter/other",
            serde_json::json!(true),
        ),
        (
            "midnight-reaper",
            "/trigger/what/filter/nonColors",
            serde_json::json!(["black"]),
        ),
        (
            "goblin-arsonist",
            "/effects/0/steps/0/optionalExtra",
            serde_json::json!(true),
        ),
        (
            "fling",
            "/effects/0/steps/0/amount/attribute",
            serde_json::json!("toughness"),
        ),
    ] {
        let raw = fixtures::SACRIFICE_RECORDS
            .iter()
            .find(|r| r.0 == name)
            .unwrap()
            .1;
        let bad = changed_source(raw, |r| {
            let index = if matches!(name, "siege-gang-commander" | "fling") {
                1
            } else {
                0
            };
            let a = &mut r["faces"][0]["result"]["abilities"][index];
            let (parent, key) = path.rsplit_once('/').unwrap();
            a.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), value);
            r["faces"][0]["result"]["lines"][index]["ability"] =
                r["faces"][0]["result"]["abilities"][index].clone();
        });
        assert!(
            !matches!(
                compile(fixtures::MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ),
            "{name}"
        );
    }
}
#[test]
fn removal_complete_source_pairs() {
    let c = embedded_catalog("removal").unwrap();
    assert_eq!(c.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        c,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-removal-ir.json"
        ))
        .unwrap()
    );
    for (name, raw) in fixtures::REMOVAL_RECORDS {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("{name}")
        };
        assert!(c.definitions.contains(&definition), "{name}");
    }
    for raw in [
        include_str!("../../../../tests/fixtures/return-to-dust.json"),
        include_str!("../../../../tests/fixtures/vandalblast.json"),
        include_str!("../../../../tests/fixtures/banishing-light.json"),
    ] {
        assert!(!matches!(
            compile(fixtures::MANIFEST, raw),
            CompileResult::Lowered { .. }
        ));
    }
    let throat = c
        .definitions
        .iter()
        .find(|d| d.name == "Go for the Throat")
        .unwrap();
    assert_eq!(
        throat.spell.as_ref().unwrap().targets[0]
            .filter
            .as_ref()
            .unwrap()
            .non_types,
        [CardKind::Artifact]
    );
    let pongify = c.definitions.iter().find(|d| d.name == "Pongify").unwrap();
    assert!(matches!(
        pongify.spell.as_ref().unwrap().instructions[0],
        kernel!(Destroy {
            regeneration_forbidden: true,
            ..
        })
    ));
}
#[test]
fn removal_direct_type_conjunction_rejects_without_changing_supported_filters() {
    fn changed_removal_target(
        raw: &str,
        text: &str,
        change: impl FnOnce(&mut serde_json::Value),
    ) -> String {
        changed_source(raw, |record| {
            record["source"]["oracle_text"] = text.into();
            let result = &mut record["faces"][0]["result"];
            let target = &mut result["abilities"][0]["effects"][0]["steps"][0]["what"];
            change(target);
            let target = target.clone();
            result["lines"][0]["text"] = text.into();
            result["lines"][0]["ability"] = result["abilities"][0].clone();
            // Keep diagnostic expressions coherent with this synthetic source.
            // They remain nonexecutable; semantic lowering owns the filter.
            let path = "/faces/0/abilities/0/effects/0/steps/0/what";
            for node in record["binding"]["nodes"].as_array_mut().unwrap() {
                if node["node"] == "selection" && node["path"] == path {
                    node["expression"] = target.clone();
                }
            }
            for reference in record["binding"]["references"].as_array_mut().unwrap() {
                if reference["path"] == path {
                    reference["expression"] = target.clone();
                }
            }
        })
    }
    let catalog = embedded_catalog("removal").unwrap();
    let source = |name| {
        fixtures::REMOVAL_RECORDS
            .iter()
            .find(|r| r.0 == name)
            .unwrap()
            .1
    };
    // The retained single-type/exclusion and explicit artifact-or-enchantment
    // union remain complete, byte-exact source-to-definition positives.
    for name in ["doom-blade", "disenchant"] {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, source(name))
        else {
            panic!("complete retained source: {name}")
        };
        assert_eq!(
            catalog.definition_by_oracle_id(&definition.oracle_id),
            Some(&*definition)
        );
    }
    for (text, field, value) in [
        (
            "Destroy target artifact or enchantment you control.",
            "control",
            serde_json::json!("you"),
        ),
        (
            "Destroy target artifact creature that's an artifact or enchantment.",
            "types",
            serde_json::json!(["artifact", "creature"]),
        ),
    ] {
        let mutant = changed_removal_target(source("disenchant"), text, |target| {
            target["filter"][field] = value;
        });
        assert!(
            matches!(
                compile(fixtures::MANIFEST, &mutant),
                CompileResult::Unsupported { .. }
            ),
            "unconsumed qualified/mixed target: {text}"
        );
    }
    // ObjectFilter.types is AND (oracle-parser/src/ast.ts3248), while the
    // finite IR PermanentFilter.types is OR. CR205.2b/601.2c/608.2b require
    // the complete target criteria at announcement and resolution. This
    // rehashed source is a regression witness, not an additional card fact.
    let mutant = changed_removal_target(
        source("doom-blade"),
        "Destroy target nonblack artifact creature.",
        |target| {
            target["filter"]["types"] = serde_json::json!(["artifact", "creature"]);
        },
    );
    let result = compile(fixtures::MANIFEST, &mutant);
    assert!(
        matches!(result, CompileResult::Unsupported { .. }),
        "direct conjunctive target must reject instead of widening to artifact OR creature: {result:?}"
    );
}
#[test]
fn removal_whole_card_mutants_cannot_lose_filters_references_or_flags() {
    for (name, field, value) in [
        (
            "swords-to-plowshares",
            "/effects/0/steps/0/regenerationForbidden",
            serde_json::json!(true),
        ),
        (
            "disenchant",
            "/effects/0/steps/0/what/filter/power",
            serde_json::json!(4),
        ),
        (
            "doom-blade",
            "/effects/0/steps/0/what/filter/nonColors",
            serde_json::json!(["purple"]),
        ),
        (
            "feed-the-swarm",
            "/effects/1/steps/0/amount/attribute",
            serde_json::json!("toughness"),
        ),
        (
            "generous-gift",
            "/effects/1/steps/0/who/of",
            serde_json::json!({"ref":"them"}),
        ),
    ] {
        let raw = fixtures::REMOVAL_RECORDS
            .iter()
            .find(|r| r.0 == name)
            .unwrap()
            .1;
        let bad = changed_source(raw, |r| {
            let a = &mut r["faces"][0]["result"]["abilities"][0];
            // Paths include new fields, so use the object parent explicitly.
            let (parent, key) = field.rsplit_once('/').unwrap();
            a.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), value);
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        assert!(
            !matches!(
                compile(fixtures::MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ),
            "{name}"
        );
    }
    let c = embedded_catalog("removal").unwrap();
    let mut raw = serde_json::to_value(
        c.definitions
            .iter()
            .find(|d| d.name == "Swords to Plowshares")
            .unwrap(),
    )
    .unwrap();
    raw["spell"]["instructions"][0]["regeneration_forbidden"] = true.into();
    assert!(serde_json::from_value::<Definition>(raw).is_err());
}
#[test]
fn attachments_complete_sources_and_strict_keyword_shapes() {
    let c = embedded_catalog("attachments").unwrap();
    assert_eq!(c.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        c,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-attachments-ir.json"
        ))
        .unwrap()
    );
    for (name, raw) in fixtures::ATTACHMENT_RECORDS {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("{name}")
        };
        assert!(c.definitions.contains(&definition), "{name}");
    }
    for raw in [
        include_str!("../../../../tests/fixtures/blanchwood-armor.json"),
        include_str!("../../../../tests/fixtures/gift-of-orzhova.json"),
        include_str!("../../../../tests/fixtures/arrest.json"),
        include_str!("../../../../tests/fixtures/mask-of-memory.json"),
    ] {
        assert!(!matches!(
            compile(fixtures::MANIFEST, raw),
            CompileResult::Lowered { .. }
        ));
    }
    let get = |name| c.definitions.iter().find(|d| d.name == name).unwrap();
    assert_eq!(
        get("Pacifism").characteristics.mana_cost.generic,
        Quantity::from(1)
    );
    assert_eq!(
        get("Spidersilk Armor").characteristics.mana_cost.generic,
        Quantity::from(2)
    );
    assert!(
        matches!(&get("Lightning Greaves").activated[0].cost, ActivationCost::Mana {cost} if cost.generic==Quantity::zero())
    );
    for keyword in [
        "defender",
        "flying",
        "reach",
        "vigilance",
        "haste",
        "lifelink",
        "deathtouch",
        "trample",
        "first strike",
        "double strike",
        "menace",
        "hexproof",
        "shroud",
    ] {
        let k: KeywordAbility = serde_json::from_value(serde_json::json!(keyword)).unwrap();
        let source = SourceStaticEffect::Modify {
            what: serde_json::json!({"ref":"equipped","noun":"creature"}),
            modifiers: vec![
                serde_json::json!({"modifier":"abilities","abilities":[{"gained":"keyword","keyword":{"keyword":keyword}}]}),
            ],
        };
        assert!(
            matches!(lower_static(&source, 0).unwrap().effect, StaticEffect::ModifyCreatures {keywords,..} if keywords == [k])
        );
        let mut intrinsic = get("Storm Crow").clone();
        intrinsic.keywords = vec![k];
        intrinsic.capabilities = intrinsic.requirements();
        intrinsic.validate().unwrap();
    }
    for keyword in [KeywordAbility::Hexproof, KeywordAbility::Shroud] {
        let mut token_spell = get("Raise the Alarm").clone();
        let kernel!(CreateTokens {
            token: kir::TokenSpec::Creature { spec: token },
            ..
        }) = &mut token_spell.spell.as_mut().unwrap().instructions[0]
        else {
            panic!("token program")
        };
        token.keywords.push(keyword);
        token_spell.capabilities = token_spell.requirements();
        let mut probe = c.clone();
        probe.definitions = vec![token_spell];
        probe.validate().unwrap();
    }
}
#[test]
fn four_colors_complete_sources_and_whole_card_negative() {
    let c = embedded_catalog("four-colors").unwrap();
    assert_eq!(c.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        c,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-four-colors-ir.json"
        ))
        .unwrap()
    );
    for (name, raw) in fixtures::FOUR_COLORS_RECORDS {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("{name}")
        };
        let source: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert_eq!(source["source"]["legalities"]["commander"], "legal");
        assert_eq!(definition.name, source["source"]["name"]);
    }
    assert!(matches!(
        compile(
            fixtures::MANIFEST,
            include_str!("../../../../tests/fixtures/hunted-dragon.json")
        ),
        CompileResult::Unsupported { .. }
    ));
    let find = |name: &str| c.definitions.iter().find(|d| d.name == name).unwrap();
    assert_eq!(
        find("Caelorna, Coral Tyrant").characteristics.power,
        Some(0.into())
    );
    assert_eq!(
        find("Caelorna, Coral Tyrant").characteristics.toughness,
        Some(8.into())
    );
    assert_eq!(
        find("Rorix Bladewing").keywords,
        vec![KeywordAbility::Flying, KeywordAbility::Haste]
    );
    assert_eq!(
        find("Wall of Air").keywords,
        vec![KeywordAbility::Defender, KeywordAbility::Flying]
    );
}

#[test]
fn mana_complete_sources_and_negatives() {
    let catalog = embedded_catalog("mana").unwrap();
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        catalog,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-mana-ir.json"
        ))
        .unwrap()
    );
    for (name, raw) in fixtures::MANA_RECORDS {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("{name}")
        };
        assert!(catalog.definitions.contains(&definition), "{name}");
        let bad = changed_source(raw, |r| {
            let result = &mut r["faces"][0]["result"];
            let i = result["abilities"]
                .as_array()
                .unwrap()
                .iter()
                .position(|a| a["kind"] == "activated")
                .unwrap();
            result["abilities"][i]["extraManaClause"] = true.into();
            result["lines"][i]["ability"] = result["abilities"][i].clone();
        });
        assert!(
            !matches!(
                compile(fixtures::MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ),
            "{name}"
        );
    }
    for raw in [
        include_str!("../../../../tests/fixtures/fellwar-stone.json"),
        include_str!("../../../../tests/fixtures/talisman-of-hierarchy.json"),
        include_str!("../../../../tests/fixtures/utopia-sprawl.json"),
    ] {
        assert!(matches!(
            compile(fixtures::MANIFEST, raw),
            CompileResult::Unsupported { .. }
        ));
    }
    let find = |name: &str| catalog.definitions.iter().find(|d| d.name == name).unwrap();
    let tower = find("Command Tower");
    assert!(!tower.characteristics.basic && tower.characteristics.subtypes.is_empty());
    assert!(tower.characteristics.color_identity.is_empty());
    assert_eq!(
        find("Sol Ring").mana_abilities[0].production,
        ManaProduction::Fixed {
            symbols: vec![Color::C, Color::C]
        }
    );
    assert_eq!(
        find("Golgari Signet").mana_abilities[0]
            .costs
            .mana
            .as_ref()
            .unwrap()
            .generic,
        Quantity::from(1)
    );
    assert_eq!(find("Mind Stone").activated.len(), 1);
    assert_eq!(
        find("Birds of Paradise").keywords,
        vec![KeywordAbility::Flying]
    );
    // Rehashed near misses must fail semantic lowering, not provenance.
    for (field, value) in [
        ("who", serde_json::json!({"player":"opponent"})),
        (
            "mana",
            serde_json::json!({"mana":"any-color","amount":{"amount":"fixed","value":2}}),
        ),
    ] {
        let raw = fixtures::MANA_RECORDS[1].1;
        let bad = changed_source(raw, |r| {
            let result = &mut r["faces"][0]["result"];
            result["abilities"][0]["effects"][0]["steps"][0][field] = value;
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(!matches!(
            compile(fixtures::MANIFEST, &bad),
            CompileResult::Lowered { .. }
        ));
    }
}

#[test]
fn staples_complete_source_pairs() {
    let catalog = embedded_catalog("staples").unwrap();
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        catalog,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-staples-ir.json"
        ))
        .unwrap()
    );
    for (name, raw) in fixtures::STAPLES_RECORDS {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("{name}")
        };
        assert_eq!(
            catalog
                .definitions
                .iter()
                .find(|d| d.oracle_id == definition.oracle_id),
            Some(definition.as_ref())
        );
        let bad = changed_source(raw, |r| {
            let result = &mut r["faces"][0]["result"];
            result["abilities"][0]["unsupportedStaplesClause"] = true.into();
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            !matches!(
                compile(fixtures::MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ),
            "{name}"
        );
    }
    let raw = include_str!("../../../../tests/fixtures/wrath-of-god.json");
    assert!(
        matches!(compile(fixtures::MANIFEST,raw),CompileResult::Unsupported {reason,..} if reason.contains("regeneration"))
    );
    // Unsupported restrictions, missing shuffle, untapped entry, and extra
    // anthem grants reject complete cards, even with recomputed provenance.
    for (name, path, value) in [
        (
            "murder",
            "/faces/0/result/abilities/0/effects/0/steps/0/what/filter/colors",
            serde_json::json!(["nonblack"]),
        ),
        (
            "rampant-growth",
            "/faces/0/result/abilities/0/effects/0/steps/1/tapped",
            serde_json::json!(false),
        ),
        (
            "explosive-vegetation",
            "/faces/0/result/abilities/0/effects/0/steps/0/for/count/value",
            serde_json::json!(3),
        ),
        (
            "glorious-anthem",
            "/faces/0/result/abilities/0/effect/what/filter/control",
            serde_json::json!("opponent"),
        ),
    ] {
        let raw = fixtures::STAPLES_RECORDS
            .iter()
            .find(|(n, _)| *n == name)
            .unwrap()
            .1;
        let bad = changed_source(raw, |r| {
            if path.ends_with("/colors") {
                r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]
                    ["colors"] = value;
            } else {
                *r.pointer_mut(path).unwrap() = value;
            }
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        assert!(
            !matches!(
                compile(fixtures::MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ),
            "{name}"
        );
    }
}
#[test]
fn basic_land_split_search_complete_frozen_sources_and_near_misses() {
    let manifest = include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
    for raw in [
        include_str!("../../../../tests/fixtures/ramos/8b755881-a72d-4e21-a369-d2924eb4585a.json"),
        include_str!("../../../../tests/fixtures/ramos/1593ea18-2f2f-4ab4-83fb-6ccc0bec8a90.json"),
    ] {
        let CompileResult::Lowered { definition } = compile(manifest, raw) else {
            panic!(
                "complete frozen split-search source: {:?}",
                compile(manifest, raw)
            );
        };
        assert_eq!(CURRENT_IR_VERSION, 1);
        assert!(matches!(
            definition.spell.as_ref().unwrap().instructions.as_slice(),
            [Instruction::SearchBasicLandsSplit { .. }]
        ));
        assert!(
            definition
                .capabilities
                .contains(&Capability::BasicLandSplitSearchV1)
        );
        assert!(
            !definition
                .capabilities
                .contains(&Capability::BasicLandSearchV1)
        );
        if definition.name == "Kodama's Reach" {
            assert_eq!(definition.characteristics.subtypes, ["Arcane"]);
        }
        let mut missing = definition.clone();
        missing
            .capabilities
            .retain(|c| *c != Capability::BasicLandSplitSearchV1);
        assert!(missing.validate().is_err());
        let mut extra = definition.clone();
        extra
            .spell
            .as_mut()
            .unwrap()
            .instructions
            .push(Instruction::draw(
                PlayerExpr::you(Context::Spell),
                1.into(),
                0,
            ));
        extra.capabilities = extra.requirements().into_iter().collect();
        assert!(extra.validate().is_err());
        for (pointer, value) in [
            ("/0/who/player", serde_json::json!("opponent")),
            ("/0/who/of", serde_json::json!({"ref":"it"})),
            ("/0/zones/of/0/owner", serde_json::json!("opponent")),
            ("/0/for/count/value", serde_json::json!(1)),
            ("/0/for/upTo", serde_json::json!(false)),
            ("/0/for/filter/supertypes/0", serde_json::json!("snow")),
            ("/0/for/filter/subtypes", serde_json::json!(["Forest"])),
            ("/1/who/player", serde_json::json!("opponent")),
            ("/1/what/ref", serde_json::json!("them")),
            ("/1/what/plural", serde_json::json!(false)),
            ("/2/steps/0/what/pool/ref", serde_json::json!("it")),
            ("/2/steps/0/what/count/value", serde_json::json!(2)),
            ("/2/steps/0/tapped", serde_json::json!(false)),
            ("/2/steps/0/to/zone", serde_json::json!("hand")),
            ("/2/steps/1/what/ref", serde_json::json!("them")),
            ("/2/steps/1/to/owner", serde_json::json!("opponent")),
            ("/2/steps/1/tapped", serde_json::json!(false)),
            ("/3/who/selectionId", serde_json::json!("qualified")),
            ("/3/library/owner/player", serde_json::json!("opponent")),
        ] {
            let bad = changed_source(raw, |r| {
                let result = &mut r["faces"][0]["result"];
                let steps = &mut result["abilities"][0]["effects"][0]["steps"];
                // Insert the leaf as well as replace existing values.
                let (parent, key) = pointer.rsplit_once('/').unwrap();
                let container = steps.pointer_mut(parent).unwrap();
                if let Ok(index) = key.parse::<usize>() {
                    container[index] = value.clone();
                } else {
                    container[key] = value.clone();
                }
                result["lines"][0]["ability"] = result["abilities"][0].clone();
            });
            assert!(
                !matches!(compile(manifest, &bad), CompileResult::Lowered { .. }),
                "{pointer}"
            );
        }
        for remove in 0..4 {
            let bad = changed_source(raw, |r| {
                let result = &mut r["faces"][0]["result"];
                result["abilities"][0]["effects"][0]["steps"]
                    .as_array_mut()
                    .unwrap()
                    .remove(remove);
                result["lines"][0]["ability"] = result["abilities"][0].clone();
            });
            assert!(!matches!(
                compile(manifest, &bad),
                CompileResult::Lowered { .. }
            ));
        }
        let renamed = changed_source(raw, |r| {
            r["source"]["name"] = "Generic split witness".into();
            r["faces"][0]["result"]["name"] = "Generic split witness".into();
        });
        assert!(matches!(
            compile(manifest, &renamed),
            CompileResult::Lowered { .. }
        ));
    }
}
#[test]
fn basic_land_search_rejects_qualified_search_and_shuffle_players() {
    for (name, raw, maximum) in [
        (
            "Rampant Growth",
            include_str!("../../../../tests/fixtures/rampant-growth.json"),
            1,
        ),
        (
            "Explosive Vegetation",
            include_str!("../../../../tests/fixtures/explosive-vegetation.json"),
            2,
        ),
        (
            "Sakura-Tribe Elder",
            include_str!("../../../../tests/fixtures/sakura-tribe-elder.json"),
            1,
        ),
    ] {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("complete archived source must lower: {name}")
        };
        let program = definition
            .activated
            .first()
            .map(|a| &a.program)
            .or(definition.spell.as_ref())
            .unwrap();
        assert!(
            matches!(program.instructions.as_slice(), [Instruction::SearchBasicLands { maximum: actual, .. }] if *actual == maximum),
            "{name}"
        );
        // These are synthetic AST negatives over unchanged exact source.
        // Qualifying both actors must not bypass the actor-equality check.
        for steps in [&[0, 2][..], &[0][..], &[2][..]] {
            for (field, value) in [
                ("of", serde_json::json!({"ref":"it"})),
                ("selectionId", serde_json::json!("qualified-player")),
            ] {
                let bad = changed_source(raw, |r| {
                    let result = &mut r["faces"][0]["result"];
                    for &step in steps {
                        result["abilities"][0]["effects"][0]["steps"][step]["who"][field] =
                            value.clone();
                    }
                    result["lines"][0]["ability"] = result["abilities"][0].clone();
                });
                assert!(
                    matches!(compile(fixtures::MANIFEST, &bad), CompileResult::Unsupported { path, .. } if path == "effects.search"),
                    "{name}, {field}, steps {steps:?}"
                );
            }
        }
    }
}
#[test]
fn target_opponent_sacrifice_rejects_qualified_player_and_chooser() {
    let raw = include_str!("../../../../tests/fixtures/cruel-edict.json");
    let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
        panic!("complete archived Cruel Edict source must lower")
    };
    let program = definition.spell.as_ref().unwrap();
    assert!(matches!(
        program.instructions.as_slice(),
        [kernel!(Sacrifice {
            player: target_player!(0),
            ..
        })]
    ));
    assert_eq!(program.targets[0].domain, TargetDomain::OpponentPlayer);
    // Synthetic AST negatives: identical qualifications on both actors
    // must reject just as a qualification on either actor alone does.
    for actors in [&["who", "chosenBy"][..], &["who"][..], &["chosenBy"][..]] {
        let bad = changed_source(raw, |r| {
            let result = &mut r["faces"][0]["result"];
            for &actor in actors {
                result["abilities"][0]["effects"][0]["steps"][0][actor]["of"] =
                    serde_json::json!({"ref":"it"});
            }
            result["lines"][0]["ability"] = result["abilities"][0].clone();
        });
        assert!(
            matches!(compile(fixtures::MANIFEST, &bad), CompileResult::Unsupported { path, .. } if path == "effects.steps"),
            "qualified sacrifice actors: {actors:?}"
        );
    }
}
fn changed_record(index: usize, change: impl FnOnce(&mut serde_json::Value)) -> String {
    changed_source(fixtures::LIBRARY_RECORDS[index].1, change)
}
pub(super) fn changed_source(raw: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut r: serde_json::Value = serde_json::from_str(raw).unwrap();
    change(&mut r);
    r["sourceSha256"] = sha256(canonical(&r["source"]).unwrap().as_bytes()).into();
    r["bindingSha256"] = sha256(canonical(&r["binding"]).unwrap().as_bytes()).into();
    if r.get("resolution").is_some() {
        r["resolutionSha256"] = sha256(canonical(&r["resolution"]).unwrap().as_bytes()).into();
    }
    let abilities = serde_json::json!([r["faces"][0]["result"]["abilities"]]);
    r["astSha256"] = sha256(canonical(&abilities).unwrap().as_bytes()).into();
    canonical(&r).unwrap()
}
/// A changed source that no family supports but that is a complete,
/// self-consistent ordinary card: the block-by-block pipeline must lower
/// it, mark it composed, and admit it only through the composition matrix.
/// Callers assert the changed semantics on the returned definition.
/// Compile a hand-built row whose placeholder text no parser resolution
/// certifies; see `compose::synthetic`.
pub(crate) fn synthetic_compile(manifest: &str, record: &str) -> CompileResult {
    compose::synthetic(|| compile(manifest, record))
}
pub(crate) fn composed_near_miss(result: CompileResult) -> Definition {
    let CompileResult::Lowered { definition } = result else {
        panic!("near miss must lower: {result:?}")
    };
    assert!(definition.composed);
    assert!(
        definition
            .capabilities
            .contains(&Capability::ComposedAbilityV1)
    );
    definition.validate().unwrap();
    *definition
}
/// A program's sole target as `(domain, types, non_types, non_colors,
/// opponent, tapped)`, with an absent filter rendered as unrestricted.
pub(crate) type TargetShape = (
    TargetDomain,
    Vec<CardKind>,
    Vec<CardKind>,
    Vec<Color>,
    bool,
    bool,
);
pub(crate) fn sole_target(program: &SpellProgram) -> TargetShape {
    let [target] = program.targets.as_slice() else {
        panic!("one target: {:?}", program.targets)
    };
    match &target.filter {
        None => (target.domain, vec![], vec![], vec![], false, false),
        Some(f) => {
            assert!(f.power.is_none());
            (
                target.domain,
                f.types.clone(),
                f.non_types.clone(),
                f.non_colors.clone(),
                f.opponent,
                f.tapped,
            )
        }
    }
}
/// The program's instruction operations, in order.
pub(crate) fn ops(program: &SpellProgram) -> Vec<String> {
    program
        .instructions
        .iter()
        .map(|i| {
            serde_json::to_value(i).unwrap()["op"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}
fn match_record(name: &str) -> &'static str {
    fixtures::MATCH_RECORDS
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
}
#[test]
fn tokens_sources_and_rejection_pairs() {
    fn changed_token_source(raw: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
        changed_source(raw, |r| {
            change(r);
            let result = &mut r["faces"][0]["result"];
            let abilities = result["abilities"].as_array().unwrap().clone();
            for (i, a) in abilities.into_iter().enumerate() {
                result["lines"][i]["ability"] = a;
            }
        })
    }
    let catalog = embedded_catalog("tokens").unwrap();
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        catalog,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-tokens-ir.json"
        ))
        .unwrap()
    );
    for (name, raw) in fixtures::TOKEN_RECORDS {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("{name}")
        };
        assert_eq!(
            catalog
                .definitions
                .iter()
                .find(|d| d.oracle_id == definition.oracle_id),
            Some(definition.as_ref())
        );
        // A rejected extra sentence must never disappear behind a valid prefix.
        let bad = changed_token_source(raw, |r| {
            let a = r["faces"][0]["result"]["abilities"].as_array_mut().unwrap();
            let ability = a.iter_mut().find(|a| a.get("effects").is_some()).unwrap();
            ability["effects"][0]["steps"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"effect":"proliferate"}));
        });
        assert!(
            !matches!(
                compile(fixtures::MANIFEST, &bad),
                CompileResult::Lowered { .. }
            ),
            "{name}"
        );
    }
    let raw = include_str!("../../../../tests/fixtures/spore-burst.json");
    assert!(!matches!(
        compile(fixtures::MANIFEST, raw),
        CompileResult::Lowered { .. }
    ));
    let raw = fixtures::TOKEN_RECORDS
        .iter()
        .find(|(n, _)| *n == "raise-the-alarm")
        .unwrap()
        .1;
    for change in [
        |v: &mut serde_json::Value| v["count"]["amount"] = "variable".into(),
        |v: &mut serde_json::Value| v["token"]["types"] = serde_json::json!(["artifact"]),
        |v: &mut serde_json::Value| {
            v["token"]["keywords"] = serde_json::json!([{"keyword":"protection"}])
        },
        |v: &mut serde_json::Value| {
            v["token"]["keywords"] = serde_json::json!([{"keyword":"flying","amount":1}])
        },
    ] {
        let bad = changed_token_source(raw, |r| {
            change(&mut r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0])
        });
        assert!(!matches!(
            compile(fixtures::MANIFEST, &bad),
            CompileResult::Lowered { .. }
        ));
    }
    let raw = fixtures::TOKEN_RECORDS
        .iter()
        .find(|(n, _)| *n == "battlegrowth")
        .unwrap()
        .1;
    for counter in ["-1/-1", "flying"] {
        let bad = changed_token_source(raw, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["counter"]["type"] =
                counter.into()
        });
        assert!(!matches!(
            compile(fixtures::MANIFEST, &bad),
            CompileResult::Lowered { .. }
        ));
    }
    let own = changed_token_source(raw, |r| {
        r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["on"] =
            serde_json::json!({"ref":"this","noun":"creature"})
    });
    // A spell has no creature source to receive "this creature" counters.
    assert!(!matches!(
        compile(fixtures::MANIFEST, &own),
        CompileResult::Lowered { .. }
    ));
}
#[test]
fn combat_keywords_complete_sources() {
    let catalog = embedded_catalog("combat-keywords").unwrap();
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        catalog,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-combat-keywords-ir.json"
        ))
        .unwrap()
    );
    for (name, raw) in fixtures::COMBAT_KEYWORD_RECORDS {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("{name}")
        };
        let source: serde_json::Value = serde_json::from_str(raw).unwrap();
        let expected = source["faces"][0]["result"]["abilities"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|a| a["keywords"].as_array().unwrap())
            .map(|k| k["keyword"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            serde_json::to_value(&definition.keywords).unwrap(),
            serde_json::json!(expected)
        );
    }
}
#[test]
fn keywords_complete_sources() {
    let catalog = embedded_catalog("keywords").unwrap();
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
    for (name, raw) in fixtures::KEYWORD_RECORDS {
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
            panic!("{name}")
        };
        let source: serde_json::Value = serde_json::from_str(raw).unwrap();
        let expected = source["faces"][0]["result"]["abilities"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|a| a["keywords"].as_array().unwrap())
            .map(|k| k["keyword"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            serde_json::to_value(&definition.keywords).unwrap(),
            serde_json::json!(expected)
        );
        for keyword in [
            serde_json::json!({"keyword":"protection"}),
            serde_json::json!({"keyword":"haste","amount":1}),
        ] {
            let changed = changed_source(raw, |r| {
                r["faces"][0]["result"]["abilities"][0]["keywords"][0] = keyword;
                r["faces"][0]["result"]["lines"][0]["ability"] =
                    r["faces"][0]["result"]["abilities"][0].clone();
            });
            assert!(
                !matches!(
                    compile(fixtures::MANIFEST, &changed),
                    CompileResult::Lowered { .. }
                ),
                "{name}"
            );
        }
    }
}
#[test]
fn defender_lowers_complete_keyword() {
    let c = embedded_catalog("match").unwrap();
    assert_eq!(c.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        c,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-match-ir.json"
        ))
        .unwrap()
    );
    let d = c
        .definitions
        .iter()
        .find(|d| d.name == "Wall of Wood")
        .unwrap();
    assert_eq!(d.keywords, vec![KeywordAbility::Defender]);
    assert!(d.capabilities.contains(&Capability::DefenderV1));
    assert_eq!(
        (
            d.characteristics.power.clone(),
            d.characteristics.toughness.clone()
        ),
        (Some(0.into()), Some(3.into()))
    );
    assert_eq!(
        d.characteristics.mana_cost,
        ManaCost {
            generic: 0.into(),
            colored: vec![Color::G]
        }
    );
    assert_eq!(d.characteristics.subtypes, ["Wall"]);
    assert!(d.activated.is_empty() && d.triggered.is_empty() && d.static_abilities.is_empty());
    assert_eq!(
        d.spell.as_ref().unwrap().resolution,
        SpellResolution::Permanent
    );
    assert!(matches!(
        compile(fixtures::MANIFEST, match_record("wall-of-vines")),
        CompileResult::Lowered { .. }
    ));
    let raw = changed_source(match_record("wall-of-wood"), |r| {
        r["source"]["name"] = "Source shape independent".into();
        r["faces"][0]["result"]["name"] = "Source shape independent".into();
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &raw),
        CompileResult::Lowered { .. }
    ));
}
#[test]
fn defender_whole_source_and_ir_fields_reject_unknown_keyword_semantics() {
    type Mutation = fn(&mut serde_json::Value);
    for mutate in [
        (|a: &mut serde_json::Value| a["keywords"][0]["keyword"] = "ward".into()) as Mutation,
        |a| a["keywords"][0]["condition"] = true.into(),
        |a| a["keywords"].as_array_mut().unwrap().clear(),
        |a| {
            a["keywords"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"keyword":"protection"}))
        },
    ] {
        let raw = changed_source(match_record("wall-of-wood"), |r| {
            mutate(&mut r["faces"][0]["result"]["abilities"][0]);
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        assert!(!matches!(
            compile(fixtures::MANIFEST, &raw),
            CompileResult::Lowered { .. }
        ));
    }
    let c = embedded_catalog("match").unwrap();
    let wall = c
        .definitions
        .iter()
        .find(|d| !d.keywords.is_empty())
        .unwrap();
    let mut bad = wall.clone();
    bad.keywords.clear();
    assert!(bad.validate().is_err());
    let mut bad = wall.clone();
    bad.characteristics.kind = CardKind::Instant;
    bad.capabilities = bad.requirements();
    assert!(bad.validate().is_err());
    let mut raw = serde_json::to_value(wall).unwrap();
    raw["keywords"] = serde_json::json!(["protection"]);
    assert!(strict_read::<Definition>(&canonical(&raw).unwrap()).is_err());
    raw["keywords"] = serde_json::json!([{"defender":null}]);
    assert!(strict_read::<Definition>(&canonical(&raw).unwrap()).is_err());
}
fn choice_record(name: &str) -> &'static str {
    fixtures::CHOICE_RECORDS
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
}
#[test]
fn complete_choices_sources_preserve_occurrences_modes_and_conditions() {
    let c = embedded_catalog("choices").unwrap();
    assert_eq!(
        c,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-choices-ir.json"
        ))
        .unwrap()
    );
    assert_eq!(c.schema_version, CURRENT_IR_VERSION);
    let seeds = c
        .definitions
        .iter()
        .find(|d| d.name == "Seeds of Strength")
        .unwrap();
    let p = seeds.spell.as_ref().unwrap();
    assert_eq!(
        p.targets
            .iter()
            .map(|t| (&*t.selection_id, t.slot))
            .collect::<Vec<_>>(),
        [("line:0:0", 0), ("line:1:0", 1), ("line:2:0", 2)]
    );
    for (i, op) in p.instructions.iter().enumerate() {
        assert!(
            matches!(op,kernel!(ModifyUntil { what: target_object!(target_slot), change: kir::Change::PowerToughness { power: fixed!(power), toughness: fixed!(toughness) }, until: kir::Until::EndOfTurn, .. } @ source_block)
            if *target_slot==i as u32 && *source_block==i as u32 && *power==Quantity::from(1) && *toughness==Quantity::from(1))
        );
    }
    let salve = c
        .definitions
        .iter()
        .find(|d| d.name == "Healing Salve")
        .unwrap();
    assert_eq!(
        salve.characteristics.mana_cost,
        ManaCost {
            generic: 0.into(),
            colored: vec![Color::W]
        }
    );
    let p = salve.spell.as_ref().unwrap();
    assert!(p.instructions.is_empty() && p.targets.is_empty());
    assert_eq!(p.modes.len(), 2);
    assert!(
        matches!(p.modes[0].program.instructions.as_slice(),[kernel!(GainLife { player: target_player!(0), amount: fixed!(amount) } @ 0)] if *amount==Quantity::from(3))
    );
    assert_eq!(
        p.modes[0].program.targets[0].domain,
        TargetDomain::AnyPlayer
    );
    assert!(
        matches!(p.modes[1].program.instructions.as_slice(),[Instruction::PreventNextDamage{target_slot:0,amount,duration:Duration::UntilEndOfTurn,source_block:0}] if *amount==Quantity::from(3))
    );
    assert_eq!(
        p.modes[1].program.targets[0].domain,
        TargetDomain::AnyDamageTarget
    );
    for cap in [
        Capability::ModalChoiceV1,
        Capability::LifeGainV1,
        Capability::DamageBudgetPreventionV1,
        Capability::AnyPlayerTargetV1,
        Capability::AnyDamageTargetV1,
    ] {
        assert!(salve.capabilities.contains(&cap));
    }
    let b = c
        .definitions
        .iter()
        .find(|d| d.name == "Slaughterhouse Bouncer")
        .unwrap();
    assert_eq!(
        b.characteristics.mana_cost,
        ManaCost {
            generic: 4.into(),
            colored: vec![Color::B]
        }
    );
    assert_eq!(b.triggered[0].condition, TriggerCondition::SelfDies);
    assert_eq!(
        b.triggered[0].intervening_if,
        Some(InterveningCondition::ControllerHandEmpty)
    );
    assert!(
        matches!(b.triggered[0].program.instructions.as_slice(),[kernel!(ModifyUntil { what: target_object!(0), change: kir::Change::PowerToughness { power: fixed!(power), toughness: fixed!(toughness) }, until: kir::Until::EndOfTurn, .. } @ 0)] if *power==Quantity::from(-3) && *toughness==Quantity::from(-3))
    );
    assert_eq!(Catalog::read(&canonical(&c).unwrap()).unwrap(), c);
    for name in ["boros-charm", "lavaborn-muse"] {
        assert!(
            matches!(
                compile(fixtures::MANIFEST, choice_record(name)),
                CompileResult::Unsupported { .. }
            ),
            "{name}"
        );
    }
    assert!(
        embedded_catalog("effects")
            .unwrap()
            .definitions
            .iter()
            .all(|d| d.name != "Healing Salve")
    );
}
#[test]
fn choices_rehashed_source_mutations_cannot_drop_modes_conditions_or_target_occurrences() {
    type Mutation = (&'static str, fn(&mut serde_json::Value));
    let cases: Vec<Mutation> = vec![
        ("healing-salve", |a| {
            a["effects"][0]["steps"][0]["count"]["exactly"] = 2.into()
        }),
        ("healing-salve", |a| {
            a["effects"][0]["steps"][0]["options"][1]["effects"][0]["steps"][0]["prevention"]["kind"] =
                "combat".into()
        }),
        ("healing-salve", |a| {
            a["effects"][0]["steps"][0]["options"][1]["effects"][0]["steps"][0]["prevention"]["shield"]
                ["amount"]["value"] = 0.into()
        }),
        ("healing-salve", |a| {
            a["effects"][0]["steps"][0]["options"]
                .as_array_mut()
                .unwrap()
                .truncate(1)
        }),
        ("healing-salve", |a| {
            a["effects"][0]["steps"][0]["options"][1]["extra"] = true.into()
        }),
        ("slaughterhouse-bouncer", |a| {
            a["condition"]["comparison"]["op"] = "le".into()
        }),
        ("slaughterhouse-bouncer", |a| {
            a["condition"]["comparison"]["value"]["value"] = 1.into()
        }),
        ("slaughterhouse-bouncer", |a| {
            a["condition"]["who"]["player"] = "owner".into()
        }),
        ("slaughterhouse-bouncer", |a| {
            a["condition"]["comparison"]["value"]["amount"] = "X".into()
        }),
        ("slaughterhouse-bouncer", |a| {
            a["condition"]["hidden"] = "lost".into()
        }),
        ("slaughterhouse-bouncer", |a| {
            a["trigger"]["what"]["ref"] = "other".into()
        }),
    ];
    for (name, mutate) in cases {
        let raw = changed_source(choice_record(name), |r| {
            mutate(&mut r["faces"][0]["result"]["abilities"][0]);
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        assert!(
            !matches!(
                compile(fixtures::MANIFEST, &raw),
                CompileResult::Lowered { .. }
            ),
            "{name}"
        );
    }
    let alias = changed_source(choice_record("seeds-of-strength"), |r| {
        r["faces"][0]["result"]["abilities"][1]["effects"][0]["steps"][0]["what"]["selectionId"] =
            "line:0:0".into();
        r["faces"][0]["result"]["lines"][1]["ability"] =
            r["faces"][0]["result"]["abilities"][1].clone();
    });
    assert!(!matches!(
        compile(fixtures::MANIFEST, &alias),
        CompileResult::Lowered { .. }
    ));
    for name in ["healing-salve", "slaughterhouse-bouncer"] {
        let renamed = changed_source(choice_record(name), |r| {
            r["source"]["name"] = "Independent shape witness".into();
            r["faces"][0]["result"]["name"] = "Independent shape witness".into();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &renamed),
            CompileResult::Lowered { .. }
        ));
    }
}
#[test]
fn choices_ir_rejects_nested_modes_and_inconsistent_children() {
    let c = embedded_catalog("choices").unwrap();
    let salve = c
        .definitions
        .iter()
        .find(|d| d.name == "Healing Salve")
        .unwrap();
    let mut cases = vec![];
    let mut d = salve.clone();
    d.spell
        .as_mut()
        .unwrap()
        .instructions
        .push(Instruction::draw(
            PlayerExpr::you(Context::Spell),
            1.into(),
            0,
        ));
    cases.push(d);
    let mut d = salve.clone();
    d.spell.as_mut().unwrap().modes.truncate(1);
    cases.push(d);
    let mut d = salve.clone();
    d.spell.as_mut().unwrap().modes[0].program.timing = SpellTiming::MainPhase;
    cases.push(d);
    let mut d = salve.clone();
    let root = d.spell.as_ref().unwrap().clone();
    d.spell.as_mut().unwrap().modes[0].program = root;
    cases.push(d);
    let mut d = salve.clone();
    d.spell.as_mut().unwrap().modes[0]
        .program
        .instructions
        .clear();
    cases.push(d);
    for mut d in cases {
        d.capabilities = d.requirements();
        assert!(d.validate().is_err());
    }
    let mut raw = serde_json::to_value(salve).unwrap();
    raw["spell"]["modes"][0]["unexpected"] = true.into();
    assert!(strict_read::<Definition>(&canonical(&raw).unwrap()).is_err());
}
fn effect_record(name: &str) -> &'static str {
    fixtures::EFFECT_RECORDS
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
}
fn effect_mutant(name: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
    changed_source(effect_record(name), |r| {
        change(&mut r["faces"][0]["result"]["abilities"][0]);
        r["faces"][0]["result"]["lines"][0]["ability"] =
            r["faces"][0]["result"]["abilities"][0].clone();
    })
}
#[test]
fn controlled_creature_replacement_lowers_complete_gratuitous_source_without_rewriting_furnace() {
    let raw = effect_record("gratuitous-violence");
    let source: serde_json::Value = serde_json::from_str(raw).unwrap();
    let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
        panic!("complete retained Gratuitous Violence must lower");
    };
    assert_eq!(definition.name, "Gratuitous Violence");
    assert_eq!(
        source["source"]["oracle_text"],
        "If a creature you control would deal damage to a permanent or player, it deals double that damage instead."
    );
    assert_eq!(definition.characteristics.kind, CardKind::Enchantment);
    assert_eq!(
        definition.characteristics.mana_cost,
        ManaCost {
            generic: 2.into(),
            colored: vec![Color::R; 3]
        }
    );
    assert_eq!(
        definition.spell.as_ref().unwrap().resolution,
        SpellResolution::Permanent
    );
    assert_eq!(
        definition.static_abilities,
        [StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::DoubleControllerCreatureDamageToPermanentOrPlayer
        }]
    );
    assert!(
        definition.activated.is_empty()
            && definition.triggered.is_empty()
            && definition.keywords.is_empty()
    );
    assert!(
        definition
            .capabilities
            .contains(&Capability::ControlledCreatureDamageReplacementV1)
    );
    assert!(
        !definition
            .capabilities
            .contains(&Capability::StaticDamageReplacementV1)
    );
    assert_eq!(definition.capabilities, definition.requirements());
    assert_eq!(definition.provenance.source_sha256, source["sourceSha256"]);
    assert_eq!(definition.provenance.ast_sha256, source["astSha256"]);
    assert_eq!(
        definition.provenance.binding_sha256,
        source["bindingSha256"]
    );
    assert_eq!(source["binding"]["fullyBound"], false);
    assert!(
        !source["binding"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    Catalog {
        schema_version: CURRENT_IR_VERSION,
        rules_edition: RULES_EDITION.into(),
        rules_sha256: RULES_SHA256.into(),
        definitions: vec![definition.as_ref().clone()],
    }
    .validate()
    .unwrap();
    for reversed in [false, true] {
        let synthetic = effect_mutant("gratuitous-violence", |a| {
            if reversed {
                a["effect"]["replacement"]["event"]["to"]
                    .as_array_mut()
                    .unwrap()
                    .reverse();
            }
        });
        let CompileResult::Lowered { definition: order } = compile(fixtures::MANIFEST, &synthetic)
        else {
            panic!("equivalent recipient union rejected");
        };
        assert_eq!(order.static_abilities, definition.static_abilities);
        assert_eq!(order.characteristics, definition.characteristics);
    }
    // The global source retains its complete definition,
    // including original provenance and capabilities, without regeneration.
    let old = embedded_catalog("effects").unwrap();
    let expected = old
        .definitions
        .iter()
        .find(|d| d.name == "Furnace of Rath")
        .unwrap();
    let CompileResult::Lowered {
        definition: furnace,
    } = compile(fixtures::MANIFEST, effect_record("furnace-of-rath"))
    else {
        panic!("Furnace source");
    };
    assert_eq!(furnace.as_ref(), expected);
    assert!(
        !furnace
            .capabilities
            .contains(&Capability::ControlledCreatureDamageReplacementV1)
    );
}

#[test]
fn controlled_creature_replacement_rejects_partial_sources_and_qualified_recipients() {
    type Mutation = fn(&mut serde_json::Value);
    for mutate in [
        (|r: &mut serde_json::Value| r["event"]["source"]["filter"]["control"] = "opponent".into())
            as Mutation,
        |r| r["event"]["source"]["filter"]["control"] = serde_json::Value::Null,
        |r| {
            r["event"]["source"]["filter"]
                .as_object_mut()
                .unwrap()
                .remove("control");
        },
        |r| r["event"]["source"]["filter"]["types"] = serde_json::json!(["artifact"]),
        |r| r["event"]["source"]["filter"]["types"] = serde_json::json!(["artifact", "creature"]),
        |r| r["event"]["source"]["filter"]["types"] = serde_json::json!(["creature", "creature"]),
        |r| r["event"]["source"]["filter"]["types"] = serde_json::json!([]),
        |r| r["event"]["source"]["filter"]["types"] = serde_json::Value::Null,
        |r| r["event"]["source"]["filter"]["owner"] = "you".into(),
        |r| r["event"]["source"]["filter"]["subtypes"] = serde_json::json!(["Goblin"]),
        |r| r["event"]["source"]["filter"]["other"] = true.into(),
        |r| r["event"]["source"]["filter"]["properties"] = serde_json::json!([]),
        |r| r["event"]["source"]["filter"]["status"] = serde_json::json!(["attacking"]),
        |r| r["event"]["source"]["filter"] = serde_json::Value::Null,
        |r| r["event"]["source"]["ref"] = "this".into(),
        |r| r["event"]["source"]["count"]["value"] = 0.into(),
        |r| r["event"]["source"]["count"]["value"] = 2.into(),
        |r| r["event"]["source"]["count"]["amount"] = "that-much".into(),
        |r| r["event"]["source"]["selectionId"] = "unbound".into(),
        |r| r["event"]["kind"] = "combat".into(),
        |r| r["event"]["to"][1]["of"] = serde_json::json!({"ref":"it"}),
        |r| r["event"]["to"][1]["selectionId"] = "unbound-player".into(),
        |r| r["event"]["to"][0]["filter"]["types"] = serde_json::json!(["creature"]),
        |r| r["event"]["to"] = serde_json::json!([{"player":"a-player"}]),
        |r| r["occurrence"] = "next".into(),
        |r| r["replace"]["change"]["factor"] = 3.into(),
        |r| r["replace"]["change"]["operation"] = "add".into(),
        |r| r["replace"]["change"]["optional"] = true.into(),
    ] {
        let raw = effect_mutant("gratuitous-violence", |a| {
            mutate(&mut a["effect"]["replacement"])
        });
        assert!(
            matches!(
                compile(fixtures::MANIFEST, &raw),
                CompileResult::Unsupported { .. }
            ),
            "unsupported complete replacement must reject"
        );
    }
}
#[test]
fn furnace_replacement_rejects_qualified_player_scope_in_either_union_order() {
    let catalog = embedded_catalog("effects").unwrap();
    let expected = catalog
        .definitions
        .iter()
        .find(|d| d.name == "Furnace of Rath")
        .unwrap();
    let CompileResult::Lowered { definition } =
        compile(fixtures::MANIFEST, effect_record("furnace-of-rath"))
    else {
        panic!("complete frozen Furnace of Rath must lower");
    };
    assert_eq!(definition.as_ref(), expected);
    for reversed in [false, true] {
        let raw = effect_mutant("furnace-of-rath", |a| {
            if reversed {
                a["effect"]["replacement"]["event"]["to"]
                    .as_array_mut()
                    .unwrap()
                    .reverse();
            }
        });
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &raw) else {
            panic!("equivalent complete recipient union rejected: reversed={reversed}");
        };
        assert_eq!(definition.static_abilities, expected.static_abilities);
        assert_eq!(definition.characteristics, expected.characteristics);
        assert_eq!(definition.capabilities, expected.capabilities);
        for (field, value) in [
            ("of", serde_json::json!({"ref":"it"})),
            ("selectionId", serde_json::json!("unbound-player")),
        ] {
            // Synthetic near-miss of the exact archived Furnace source:
            // neither a qualified object relation nor a selected player is
            // represented by the all-permanent/player doubling instruction.
            let qualified = effect_mutant("furnace-of-rath", |a| {
                let to = a["effect"]["replacement"]["event"]["to"]
                    .as_array_mut()
                    .unwrap();
                let player = to.iter_mut().find(|r| r["player"] == "a-player").unwrap();
                player[field] = value;
                if reversed {
                    to.reverse();
                }
            });
            assert!(
                matches!(
                    compile(fixtures::MANIFEST, &qualified),
                    CompileResult::Unsupported { .. }
                ),
                "unsupported player {field} must not be dropped: reversed={reversed}"
            );
        }
    }
}
#[test]
fn complete_effects_catalog_binds_all_six_sources() {
    let c = embedded_catalog("effects").unwrap();
    assert_eq!(c.schema_version, CURRENT_IR_VERSION);
    for (name, power, toughness) in [("Giant Growth", 3, 3), ("Disfigure", -2, -2)] {
        let d = c.definitions.iter().find(|d| d.name == name).unwrap();
        assert!(matches!(d.spell.as_ref().unwrap().instructions.as_slice(),
            [i] if i.pumped().is_some_and(|(what, p, t, block)| *what == kir::SetExpr::target(0)
                && block == 0 && *p == Quantity::from(power) && *t == Quantity::from(toughness))));
        assert_eq!(
            d.spell.as_ref().unwrap().targets[0].domain,
            TargetDomain::BattlefieldCreature
        );
    }
    let fog = c.definitions.iter().find(|d| d.name == "Fog").unwrap();
    assert!(fog.spell.as_ref().unwrap().targets.is_empty());
    assert!(matches!(
        fog.spell.as_ref().unwrap().instructions.as_slice(),
        [Instruction::PreventCombatDamage {
            duration: Duration::UntilEndOfTurn,
            source_block: 0
        }]
    ));
    let healer = c
        .definitions
        .iter()
        .find(|d| d.name == "Samite Healer")
        .unwrap();
    assert_eq!(healer.activated[0].cost, ActivationCost::TapSource);
    assert_eq!(
        healer.activated[0].program.targets[0].domain,
        TargetDomain::AnyDamageTarget
    );
    assert!(
        matches!(healer.activated[0].program.instructions.as_slice(),[Instruction::PreventNextDamage { target_slot:0,amount,duration:Duration::UntilEndOfTurn,source_block:0 }] if *amount==Quantity::from(1))
    );
    let furnace = c
        .definitions
        .iter()
        .find(|d| d.name == "Furnace of Rath")
        .unwrap();
    assert_eq!(furnace.characteristics.kind, CardKind::Enchantment);
    assert_eq!(
        furnace.characteristics.mana_cost,
        ManaCost {
            generic: Quantity::from(1),
            colored: vec![Color::R; 3]
        }
    );
    assert_eq!(
        furnace.static_abilities,
        vec![StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::DoubleDamageToPermanentOrPlayer
        }]
    );
    assert_eq!(
        furnace.spell.as_ref().unwrap().resolution,
        SpellResolution::Permanent
    );
    let ox = c.definitions.iter().find(|d| d.name == "Yoked Ox").unwrap();
    assert_eq!(
        (
            ox.characteristics.power.clone(),
            ox.characteristics.toughness.clone()
        ),
        (Some(Quantity::from(0)), Some(Quantity::from(4)))
    );
    assert_eq!(Catalog::read(&canonical(&c).unwrap()).unwrap(), c);
    assert_eq!(
        c,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-effects-ir.json"
        ))
        .unwrap()
    );
    for (name, generic, colored) in [
        ("Giant Growth", 0, vec![Color::G]),
        ("Disfigure", 0, vec![Color::B]),
        ("Fog", 0, vec![Color::G]),
        ("Samite Healer", 1, vec![Color::W]),
        ("Yoked Ox", 0, vec![Color::W]),
    ] {
        assert_eq!(
            c.definitions
                .iter()
                .find(|d| d.name == name)
                .unwrap()
                .characteristics
                .mana_cost,
            ManaCost {
                generic: generic.into(),
                colored
            }
        );
    }
}
fn research_thief_event_it_mutation() -> String {
    // Synthetic body parsed and reference-analyzed by the TypeScript source
    // adapter during defect triage; this is not an additional card fact.
    let raw = include_str!(
        "../../../../tests/fixtures/cards-data-d2ce/ba5055ce-6809-47a4-bee6-fb97a5b60f8a.json"
    );
    changed_source(raw, |record| {
        let text = "Whenever an artifact creature you control deals combat damage to a player, it deals 1 damage to each opponent.";
        record["source"]["oracle_text"] = format!("Flash\nFlying\n{text}").into();
        let result = &mut record["faces"][0]["result"];
        result["abilities"][2]["effects"][0]["steps"][0] = serde_json::json!({
            "effect":"damage","source":{"ref":"it"},"amount":{"amount":"fixed","value":1},
            "to":{"to":"player","ref":{"player":"each-opponent"}}
        });
        result["lines"][2]["text"] = text.into();
        result["lines"][2]["ability"] = result["abilities"][2].clone();
        // Mirror the actual analyzeCardBindings result: It denotes the
        // triggering objects, while the recipient is a resolution query.
        // The compiler still must prove executable source equality itself.
        let prefix = format!("card:{}", record["oracleIdentity"].as_str().unwrap());
        let ability = format!("{prefix}/faces/0/abilities/2#ability");
        let step = "/faces/0/abilities/2/effects/0/steps/0";
        let event = format!("{prefix}{step}#event");
        let source = format!("{prefix}/faces/0/abilities/2/trigger#objects");
        let recipient = format!("{prefix}{step}/to/ref#query");
        let nodes = record["binding"]["nodes"].as_array_mut().unwrap();
        nodes.retain(|node| !node["path"].as_str().unwrap().starts_with(step));
        nodes.extend(serde_json::from_value::<Vec<serde_json::Value>>(serde_json::json!([
            {"id":recipient,"path":format!("{step}/to/ref"),"node":"selection","scope":ability,
                "expression":{"player":"each-opponent"},"cardinality":"many","timing":"resolution",
                "selection":"evaluate-reference","rules":["608.2h"]},
            {"id":event,"path":step,"node":"event","scope":ability,"operation":"damage",
                "inputs":[source,recipient],"availability":"when-performed","rules":["608.2c"]},
            {"id":format!("{prefix}{step}#quantity"),"path":step,"node":"event-result","event":event,
                "result":"quantity","cardinality":"unknown","availability":"if-produced",
                "identity":"measured-quantity","rules":["608.2h"]}
        ])).unwrap());
        let references = record["binding"]["references"].as_array_mut().unwrap();
        references.retain(|reference| !reference["path"].as_str().unwrap().starts_with(step));
        references.extend(serde_json::from_value::<Vec<serde_json::Value>>(serde_json::json!([
            {"path":format!("{step}/source"),"expression":{"ref":"it"},"binding":"bound","target":source,
                "rules":["608.2k","400.7j","608.2h"]},
            {"path":format!("{step}/to/ref"),"expression":{"player":"each-opponent"},"binding":"bound",
                "target":recipient,"rules":["608.2h"]}
        ])).unwrap());
        record["binding"]["counts"] = serde_json::json!({"obligations":4,"bound":4,"unresolved":0});
    })
}
#[test]
fn filtered_damage_trigger_cannot_rebind_event_it_to_observer_source() {
    let mutant = research_thief_event_it_mutation();
    let record: serde_json::Value = strict_read(&mutant).unwrap();
    assert!(
        record["binding"]["references"]
            .as_array()
            .unwrap()
            .iter()
            .any(
                |reference| reference["path"] == "/faces/0/abilities/2/effects/0/steps/0/source"
                    && reference["expression"] == serde_json::json!({"ref":"it"})
                    && reference["target"]
                        .as_str()
                        .unwrap()
                        .ends_with("/trigger#objects")
            )
    );
    // CR120.2b/608.2h: the specified object deals damage, not the observing
    // ability/source. No current opcode carries the filtered event object.
    let result = compile(
        include_str!("../../../../tests/fixtures/cards-data-d2ce/archive-manifest-current.json"),
        &mutant,
    );
    assert!(
        matches!(result, CompileResult::Unsupported { .. }),
        "filtered event It must not become observer-source damage: {result:?}"
    );
}
#[test]
fn trigger_damage_it_requires_first_step_self_event_scope() {
    // Synthetic binder probes are separate from the full-source Research
    // regression above. No archived source or expected IR is rewritten.
    let record: serde_json::Value = strict_read(&research_thief_event_it_mutation()).unwrap();
    let Ability::Triggered {
        effects, trigger, ..
    } = serde_json::from_value(record["faces"][0]["result"]["abilities"][2].clone()).unwrap()
    else {
        panic!("trigger source")
    };
    let mut explicit = effects.clone();
    let Step::Damage { source, .. } = &mut explicit[0].steps[0] else {
        panic!("damage")
    };
    *source = DamageSource::This {
        noun: "creature".into(),
    };
    let this = serde_json::json!({"ref":"this","noun":"creature"});
    let other = serde_json::json!({"ref":"filter","filter":{"types":["creature"],"other":true},"count":{"amount":"fixed","value":1}});
    let filtered = serde_json::json!({"ref":"filter","filter":{"types":["creature"],"control":"you"},"count":{"amount":"fixed","value":1}});
    let mut unsafe_contexts = vec![*trigger];
    for value in [
        serde_json::json!({"trigger":"enters","what":other}),
        serde_json::json!({"trigger":"any-of","events":[{"trigger":"dies","what":this},{"trigger":"dies","what":other}]}),
        serde_json::json!({"trigger":"dies","what":filtered}),
        serde_json::json!({"trigger":"dies","what":{"ref":"filter","filter":{"types":["creature"],"control":"opponent"},"count":{"amount":"fixed","value":1}}}),
        serde_json::json!({"trigger":"cast","who":{"player":"a-player"},"what":{"cls":"spell"}}),
        serde_json::json!({"trigger":"phase","phase":"upkeep","whose":"each-player"}),
        serde_json::json!({"trigger":"attacks","what":filtered}),
    ] {
        unsafe_contexts.push(serde_json::from_value(value).unwrap());
    }
    for trigger in unsafe_contexts {
        assert!(
            matches!(lower_trigger(&trigger, &effects, None, 2),
            Err(Failure { unsupported: true, ref path, .. }) if path == "abilities.effects.source"),
            "{trigger:?}"
        );
        // Explicit this-creature damage continues to mean the invocation's
        // source, independently of which other object caused the event.
        assert!(
            lower_trigger(&trigger, &explicit, None, 2).is_ok(),
            "{trigger:?}"
        );
    }
    let safe_contexts: Vec<SourceTrigger> = [
        serde_json::json!({"trigger":"dies","what":this}),
        serde_json::json!({"trigger":"enters","what":this}),
        serde_json::json!({"trigger":"attacks","what":this}),
        serde_json::json!({"trigger":"deals-damage","what":this,"combat":true,"to":"any"}),
    ]
    .into_iter()
    .map(|value| serde_json::from_value(value).unwrap())
    .collect();
    for trigger in &safe_contexts {
        assert!(
            lower_trigger(trigger, &effects, None, 2).is_ok(),
            "{trigger:?}"
        );
    }
    let draw = Step::Draw {
        who: PlayerSelector {
            player: "you".into(),
            selection_id: None,
            of: None,
        },
        amount: DrawAmount::Fixed { value: 1 },
        optional: false,
    };
    let mut later_step = effects.clone();
    later_step[0].steps.insert(0, draw.clone());
    let mut later_sentence = effects.clone();
    later_sentence.insert(
        0,
        Sentence {
            condition: None,
            condition_kind: None,
            bindings: vec![],
            steps: vec![draw],
        },
    );
    let modal = vec![Sentence {
        condition: None,
        condition_kind: None,
        bindings: vec![],
        steps: vec![Step::Modal {
            count: ModeCount::Exact { exactly: 1 },
            options: vec![
                ModeOption {
                    effects: effects.clone()
                };
                2
            ],
        }],
    }];
    for effects in [later_step, later_sentence, modal] {
        assert!(
            matches!(lower_trigger(&safe_contexts[0], &effects, None, 2),
            Err(Failure { unsupported: true, ref path, .. }) if path == "abilities.effects.source")
        );
    }
    let grouped: SourceTrigger = serde_json::from_value(serde_json::json!({
        "trigger":"deals-damage","what":{"ref":"filter","filter":{"types":["creature"],"control":"you",
            "properties":[{"property":"power","comparison":{"op":"gt","value":{"amount":"attribute","of":{"ref":"it"},"attribute":"base-power"}}}]},
            "count":{"amount":"fixed","value":1}},"combat":true,"to":"any","grouping":"one-or-more"
    })).unwrap();
    assert!(matches!(lower_trigger(&grouped, &effects, None, 2),
        Err(Failure { unsupported: true, ref path, .. }) if path == "abilities.effects.source"));
}
#[test]
fn trigger_damage_it_preserves_retained_self_source_classification() {
    for raw in [
        include_str!("../../../../tests/fixtures/goblin-arsonist.json"),
        include_str!("../../../../tests/fixtures/mogg-fanatic.json"),
    ] {
        assert!(matches!(
            compile(fixtures::MANIFEST, raw),
            CompileResult::Lowered { .. }
        ));
    }
    let manifest =
        include_str!("../../../../tests/fixtures/cards-data-d2cd/archive-manifest-current.json");
    for raw in [
        include_str!(
            "../../../../tests/fixtures/cards-data-d2cd/3d563149-7d4e-43e5-b701-28e3212051ec.json"
        ),
        include_str!(
            "../../../../tests/fixtures/cards-data-d2cd/47e33aa1-2f54-471c-a484-de13bcd2dceb.json"
        ),
        include_str!(
            "../../../../tests/fixtures/cards-data-d2cd/bbd84035-3efd-4301-9778-913898efd57a.json"
        ),
    ] {
        assert!(matches!(
            compile(manifest, raw),
            CompileResult::Lowered { .. }
        ));
    }
    // Falkenrath Perforator's source pronoun is valid, but its complete
    // defending-player recipient remains unsupported. The existing D2c-d
    // source classifier already rejects archive line 16623 for this reason.
    let perforator = compile(
        manifest,
        include_str!(
            "../../../../tests/fixtures/cards-data-d2cd/6e991808-fea4-46a8-9e87-7b5917f33ce8.json"
        ),
    );
    assert!(
        matches!(&perforator, CompileResult::Unsupported { reason, .. }
            if reason.contains("unsupported complete damage recipient")),
        "{perforator:?}"
    );
}
#[test]
fn blocking_creature_pump_lowers_complete_righteousness_source() {
    let raw = effect_record("righteousness");
    assert_eq!(
        sha256(raw.trim_end().as_bytes()),
        "7f93ff4781abe286d05e5835771b711f964771fedddbf6e580d90c89074f62e0"
    );
    let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, raw) else {
        panic!("complete pinned Righteousness source must lower")
    };
    assert_eq!(definition.oracle_id, "3f6b2f76-0364-415e-a6a2-e9e5bf31b745");
    assert_eq!(definition.provenance.source_index, 9534);
    assert_eq!(
        definition.provenance.source_sha256,
        "f9af121b8a7525a90421c871f441e59b9449d1509a571e54b59656aca48f8177"
    );
    assert_eq!(
        definition.provenance.ast_sha256,
        "39040712bbdeb4100fbbe67e4010a216db8a3cb431bd754c078285ab3167932f"
    );
    assert_eq!(
        definition.provenance.binding_sha256,
        "eba91cf74d2af16085aab09b7a771235dede6bc6cebed8f34c3c6e05696e0351"
    );
    let record: serde_json::Value = strict_read(raw).unwrap();
    assert_eq!(
        strict_read::<serde_json::Value>(&definition.provenance.source_json).unwrap(),
        record["source"]
    );
    assert_eq!(
        strict_read::<serde_json::Value>(&definition.provenance.abilities_json).unwrap(),
        record["faces"][0]["result"]["abilities"]
    );
    assert_eq!(definition.characteristics.kind, CardKind::Instant);
    assert_eq!(
        definition.characteristics.mana_cost,
        ManaCost {
            generic: Quantity::zero(),
            colored: vec![Color::W]
        }
    );
    assert!(
        definition.activated.is_empty()
            && definition.triggered.is_empty()
            && definition.static_abilities.is_empty()
    );
    assert_eq!(
        definition.spell,
        Some(SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            timing: SpellTiming::Instant,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            modes: vec![],
            targets: vec![TargetSpec {
                slot: 0,
                selection_id: "line:0:0".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                domain: TargetDomain::BlockingBattlefieldCreature,
                filter: None
            }],
            instructions: vec![Instruction::pump(
                kir::SetExpr::target(0),
                7.into(),
                7.into(),
                0
            )],
        })
    );
    assert_eq!(
        definition.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastInstantV1,
            Capability::BlockingCreatureTargetV1,
            Capability::TemporaryPowerToughnessV1
        ])
    );
    Catalog {
        schema_version: CURRENT_IR_VERSION,
        rules_edition: RULES_EDITION.into(),
        rules_sha256: RULES_SHA256.into(),
        definitions: vec![*definition],
    }
    .validate()
    .unwrap();
}
#[test]
fn blocking_creature_pump_rejects_unconsumed_target_and_effect_qualifiers() {
    for (path, value) in [
        ("/what/filter/status", serde_json::json!([])),
        ("/what/filter/status", serde_json::json!(["attacking"])),
        ("/what/filter/status", serde_json::json!(["blocked"])),
        (
            "/what/filter/status",
            serde_json::json!(["blocking", "attacking"]),
        ),
        ("/what/filter/status", serde_json::Value::Null),
        (
            "/what/filter/types",
            serde_json::json!(["artifact", "creature"]),
        ),
        ("/what/filter/owner", serde_json::json!("you")),
        ("/what/filter/control", serde_json::json!("you")),
        ("/what/count/value", serde_json::json!(2)),
        ("/what/count/amount", serde_json::json!("x")),
        ("/what/selectionId", serde_json::json!("")),
        ("/what/ref", serde_json::json!("filter")),
        ("/optional", serde_json::json!(false)),
        ("/duration/duration", serde_json::json!("permanent")),
    ] {
        let raw = changed_source(effect_record("righteousness"), |record| {
            let result = &mut record["faces"][0]["result"];
            let step = &mut result["abilities"][0]["effects"][0]["steps"][0];
            let (parent, key) = path.rsplit_once('/').unwrap();
            step.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), value);
            let target = step["what"].clone();
            result["lines"][0]["ability"] = result["abilities"][0].clone();
            for node in record["binding"]["nodes"].as_array_mut().unwrap() {
                if node["node"] == "selection" {
                    node["expression"] = target.clone();
                }
            }
            for reference in record["binding"]["references"].as_array_mut().unwrap() {
                reference["expression"] = target.clone();
            }
        });
        assert!(
            matches!(
                compile(fixtures::MANIFEST, &raw),
                CompileResult::Unsupported { .. }
            ),
            "{path}"
        );
    }
    let with_draw = changed_source(effect_record("righteousness"), |record| {
        let text = "Target blocking creature gets +7/+7 until end of turn. Draw a card.";
        record["source"]["oracle_text"] = text.into();
        let result = &mut record["faces"][0]["result"];
        result["abilities"][0]["effects"][0]["steps"].as_array_mut().unwrap().push(
            serde_json::json!({"effect":"draw","who":{"player":"you"},"amount":{"amount":"fixed","value":1}}));
        result["lines"][0]["text"] = text.into();
        result["lines"][0]["ability"] = result["abilities"][0].clone();
    });
    assert!(
        matches!(compile(fixtures::MANIFEST, &with_draw), CompileResult::Unsupported { path, .. } if path == "definition")
    );
}
#[test]
fn blocking_creature_pump_rejects_inconsistent_reused_selection_domains() {
    let record: serde_json::Value = strict_read(effect_record("righteousness")).unwrap();
    let blocking =
        record["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"].clone();
    let mut ordinary = blocking.clone();
    ordinary["filter"].as_object_mut().unwrap().remove("status");
    for (first, second, domain) in [
        (
            &blocking,
            &ordinary,
            TargetDomain::BlockingBattlefieldCreature,
        ),
        (&ordinary, &blocking, TargetDomain::BattlefieldCreature),
    ] {
        let mut program = embedded_catalog("effects")
            .unwrap()
            .definitions
            .into_iter()
            .find(|d| d.name == "Giant Growth")
            .unwrap()
            .spell
            .unwrap();
        program.targets.clear();
        program.instructions.clear();
        let mut slots = BTreeMap::new();
        assert_eq!(
            bind_pump_target(&mut program, &mut slots, first).unwrap(),
            0
        );
        assert!(bind_pump_target(&mut program, &mut slots, second).is_err());
        assert_eq!(program.targets.len(), 1);
        assert_eq!(program.targets[0].domain, domain);
    }
}
#[test]
fn complete_effects_sources_reject_filtered_modal_and_optional_near_misses() {
    // Gratuitous Violence's original source negative is now a complete
    // controlled-creature replacement positive above; other partial sources
    // remain unsupported; each complete source retains its exact expected program.
    for name in ["flame-rift", "deathgreeter"] {
        assert!(
            matches!(
                compile(fixtures::MANIFEST, effect_record(name)),
                CompileResult::Unsupported { .. }
            ),
            "{name}"
        );
    }
    for name in ["fog", "samite-healer", "furnace-of-rath"] {
        let raw = changed_source(effect_record(name), |r| {
            r["binding"]["fullyBound"] = true.into();
            r["source"]["name"] = "Name independent source witness".into();
            r["faces"][0]["result"]["name"] = "Name independent source witness".into();
        });
        assert!(
            matches!(
                compile(fixtures::MANIFEST, &raw),
                CompileResult::Lowered { .. }
            ),
            "{name}"
        );
    }
}
#[test]
fn effects_require_exact_capability_closure() {
    let c = embedded_catalog("effects").unwrap();
    for d in c.definitions.iter().filter(|d| {
        d.capabilities.iter().any(|c| {
            matches!(
                c,
                Capability::CastEnchantmentV1
                    | Capability::TemporaryPowerToughnessV1
                    | Capability::CombatDamagePreventionV1
                    | Capability::DamageBudgetPreventionV1
                    | Capability::StaticDamageReplacementV1
            )
        })
    }) {
        for cap in &d.capabilities {
            let mut bad = d.clone();
            bad.capabilities.remove(cap);
            assert!(bad.validate().is_err());
        }
    }
}
#[test]
fn rehashed_effect_scope_duration_and_allocation_mutants_fail_complete_lowering() {
    let cases: [(&str, &str, serde_json::Value); 16] = [
        (
            "giant-growth",
            "/effects/0/steps/0/power/sign",
            serde_json::json!(2),
        ),
        (
            "disfigure",
            "/effects/0/steps/0/duration/duration",
            serde_json::json!("permanent"),
        ),
        (
            "fog",
            "/effects/0/steps/0/prevention/kind",
            serde_json::json!("all"),
        ),
        (
            "fog",
            "/effects/0/steps/0/prevention/eventScope/sources",
            serde_json::json!("separate"),
        ),
        (
            "fog",
            "/effects/0/steps/0/prevention/eventScope/recipients",
            serde_json::json!("separate"),
        ),
        (
            "fog",
            "/effects/0/steps/0/prevention/shield/occurrence",
            serde_json::json!("next"),
        ),
        (
            "samite-healer",
            "/effects/0/steps/0/prevention/kind",
            serde_json::json!("combat"),
        ),
        (
            "samite-healer",
            "/effects/0/steps/0/prevention/shield/amount/value",
            serde_json::json!(0),
        ),
        (
            "samite-healer",
            "/effects/0/steps/0/prevention/to",
            serde_json::json!([]),
        ),
        (
            "samite-healer",
            "/effects/0/steps/0/duration/duration",
            serde_json::json!("end-of-turn"),
        ),
        (
            "furnace-of-rath",
            "/effect/replacement/event/kind",
            serde_json::json!("combat"),
        ),
        (
            "furnace-of-rath",
            "/effect/replacement/event/source/filter",
            serde_json::json!({"types":["creature"]}),
        ),
        (
            "furnace-of-rath",
            "/effect/replacement/event/to",
            serde_json::json!([{"player":"a-player"},{"player":"a-player"}]),
        ),
        (
            "furnace-of-rath",
            "/effect/replacement/occurrence",
            serde_json::json!("next"),
        ),
        (
            "furnace-of-rath",
            "/effect/replacement/replace/change/operation",
            serde_json::json!("add"),
        ),
        (
            "furnace-of-rath",
            "/effect/replacement/replace/change/factor",
            serde_json::json!(3),
        ),
    ];
    for (name, path, value) in cases {
        let raw = effect_mutant(name, |ability| {
            *ability.pointer_mut(path).expect(path) = value
        });
        assert!(
            matches!(
                compile(fixtures::MANIFEST, &raw),
                CompileResult::Unsupported { .. }
            ),
            "{name} {path}"
        );
    }
    let extra = effect_mutant("samite-healer", |a| {
        a["effects"][0]["steps"][0]["prevention"]["after"] = serde_json::json!({"effect":"draw"})
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &extra),
        CompileResult::Unsupported { .. }
    ));
}
#[test]
fn effect_binder_consumes_complete_scope_fields_without_diagnostic_or_name_dispatch() {
    type Mutation = (&'static str, fn(&mut serde_json::Value));
    let cases: Vec<Mutation> = vec![
        ("giant-growth", |a| {
            a["effects"][0]["steps"][0]["what"]["filter"]["control"] = serde_json::json!("you")
        }),
        ("giant-growth", |a| {
            a["effects"][0]["steps"][0]["power"]["sign"] = serde_json::json!(0)
        }),
        ("giant-growth", |a| {
            a["effects"][0]["steps"][0]["power"]["amount"]["amount"] = serde_json::json!("variable")
        }),
        ("giant-growth", |a| {
            a["effects"][0]["steps"][0]["duration"]["unknown"] = true.into()
        }),
        ("disfigure", |a| {
            a["effects"][0]["steps"][0]["toughness"]["amount"]["value"] = (-2).into()
        }),
        ("fog", |a| {
            a["effects"][0]["steps"][0]["prevention"]["to"] = serde_json::json!([])
        }),
        ("fog", |a| {
            a["effects"][0]["steps"][0]["prevention"]["by"] = serde_json::json!({"player":"you"})
        }),
        ("fog", |a| {
            a["effects"][0]["steps"][0]["prevention"]["shield"]["prevent"]["optional"] = true.into()
        }),
        ("samite-healer", |a| {
            a["costs"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"cost":"mana","symbols":["W"]}))
        }),
        ("samite-healer", |a| {
            a["effects"][0]["steps"][0]["prevention"]["eventScope"]["recipients"] = "each".into()
        }),
        ("samite-healer", |a| {
            a["effects"][0]["steps"][0]["prevention"]["to"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"ref":"any-target","selectionId":"extra"}))
        }),
        ("samite-healer", |a| {
            a["effects"][0]["steps"][0]["prevention"]["to"][0]["ref"] = "it".into()
        }),
        ("samite-healer", |a| {
            a["effects"][0]["steps"][0]["optional"] = true.into()
        }),
        ("furnace-of-rath", |a| {
            a["effect"]["replacement"]["event"]["source"]["count"]["value"] = 2.into()
        }),
        ("furnace-of-rath", |a| {
            a["effect"]["replacement"]["event"]["to"][0]["filter"]["control"] = "you".into()
        }),
        ("furnace-of-rath", |a| {
            a["effect"]["replacement"]["event"]["to"][1]["player"] = "you".into()
        }),
        ("furnace-of-rath", |a| {
            a["effect"]["replacement"]["replace"]["change"]["extra"] = true.into()
        }),
    ];
    for (name, mutate) in cases {
        let raw = changed_source(effect_record(name), |r| {
            r["binding"]["fullyBound"] = true.into();
            mutate(&mut r["faces"][0]["result"]["abilities"][0]);
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        assert!(
            matches!(
                compile(fixtures::MANIFEST, &raw),
                CompileResult::Unsupported { .. }
            ),
            "{name}"
        );
    }
    let reversed = effect_mutant("furnace-of-rath", |a| {
        a["effect"]["replacement"]["event"]["to"]
            .as_array_mut()
            .unwrap()
            .reverse()
    });
    let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &reversed) else {
        panic!("equivalent recipient union rejected")
    };
    assert_eq!(
        definition.static_abilities,
        [StaticAbility {
            functioning_zone: FunctioningZone::Battlefield,
            effect: StaticEffect::DoubleDamageToPermanentOrPlayer
        }]
    );
    for name in ["furnace-of-rath", "samite-healer"] {
        let extra = changed_source(effect_record(name), |r| {
            let unknown =
                serde_json::json!({"kind":"keywords","keywords":[{"keyword":"protection"}]});
            r["faces"][0]["result"]["abilities"]
                .as_array_mut()
                .unwrap()
                .push(unknown.clone());
            r["faces"][0]["result"]["lines"]
                .as_array_mut()
                .unwrap()
                .push(
                    serde_json::json!({"text":"Protection from red","ok":true,"ability":unknown}),
                );
            r["source"]["oracle_text"] = format!(
                "{}\nProtection from red",
                r["source"]["oracle_text"].as_str().unwrap()
            )
            .into();
        });
        assert!(
            matches!(
                compile(fixtures::MANIFEST, &extra),
                CompileResult::Unsupported { .. }
            ),
            "whole {name}"
        );
    }
}
#[test]
fn effects_ir_rejects_unbound_targets_invalid_budgets_and_static_zones() {
    let c = embedded_catalog("effects").unwrap();
    type Mutation = (&'static str, fn(&mut serde_json::Value));
    let mutations: Vec<Mutation> = vec![
        ("Giant Growth", |d| {
            d["spell"]["instructions"][0]["target_slot"] = 1.into()
        }),
        ("Disfigure", |d| {
            d["spell"]["targets"][0]["domain"] = "any_damage_target".into()
        }),
        ("Disfigure", |d| {
            d["spell"]["instructions"][0]["power"] = "-0".into()
        }),
        ("Fog", |d| {
            d["spell"]["instructions"][0]["duration"] = "permanent".into()
        }),
        ("Fog", |d| {
            d["spell"]["instructions"][0]["target_slot"] = 0.into()
        }),
        ("Samite Healer", |d| {
            d["activated"][0]["program"]["instructions"][0]["amount"] = "0".into()
        }),
        ("Samite Healer", |d| {
            d["activated"][0]["program"]["instructions"][0]["amount"] = "-1".into()
        }),
        ("Samite Healer", |d| {
            d["activated"][0]["program"]["instructions"][0]["amount"] = "01".into()
        }),
        ("Samite Healer", |d| {
            d["activated"][0]["program"]["targets"][0]["domain"] = "battlefield_creature".into()
        }),
        ("Furnace of Rath", |d| {
            d["static_abilities"][0]["functioning_zone"] = "stack".into()
        }),
        ("Furnace of Rath", |d| {
            d["static_abilities"][0]["effect"] = "double_only_your_damage".into()
        }),
        ("Furnace of Rath", |d| {
            d["characteristics"]["subtypes"] = serde_json::json!(["Aura"])
        }),
        ("Furnace of Rath", |d| {
            d["spell"]["timing"] = "instant".into()
        }),
    ];
    for (name, mutate) in mutations {
        let mut value = serde_json::to_value(&c).unwrap();
        mutate(
            value["definitions"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|d| d["name"] == name)
                .unwrap(),
        );
        assert!(
            Catalog::read(&canonical(&value).unwrap()).is_err(),
            "{name}"
        );
    }
    let mut future = c;
    future.schema_version = mtg_contracts::CURRENT_IR_VERSION + 1;
    assert!(future.validate().is_err());
}
#[test]
fn complete_trigger_pool_binds_exact_source_roles() {
    let catalog = embedded_catalog("trigger").unwrap();
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-trigger-ir.json"
        ))
        .unwrap(),
        catalog
    );
    let get = |name: &str| catalog.definitions.iter().find(|d| d.name == name).unwrap();
    let visionary = get("Elvish Visionary");
    assert_eq!(
        visionary.characteristics.mana_cost,
        ManaCost {
            generic: 1.into(),
            colored: vec![Color::G]
        }
    );
    assert_eq!(visionary.characteristics.power, Some(1.into()));
    assert_eq!(visionary.characteristics.toughness, Some(1.into()));
    assert_eq!(
        visionary.triggered,
        [TriggeredAbility {
            death_filter: None,
            intervening_if: None,
            functioning_zone: FunctioningZone::Battlefield,
            condition: TriggerCondition::SelfEntersBattlefield,
            program: SpellProgram {
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                modes: vec![],
                functioning_zone: FunctioningZone::Stack,
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                targets: vec![],
                instructions: vec![Instruction::draw(
                    PlayerExpr::you(Context::Triggered),
                    1.into(),
                    0
                )]
            }
        }]
    );
    let warden = get("Essence Warden");
    assert_eq!(
        warden.characteristics.mana_cost,
        ManaCost {
            generic: 0.into(),
            colored: vec![Color::G]
        }
    );
    assert_eq!(
        warden.triggered[0].condition,
        TriggerCondition::OtherCreatureEntersBattlefield
    );
    assert_eq!(
        warden.triggered[0].program.instructions,
        [Instruction::act(
            Verb::GainLife {
                player: PlayerExpr::you(Context::Triggered),
                amount: Amount::fixed(1)
            },
            0
        )]
    );
    let artist = get("Blood Artist");
    assert_eq!(
        artist.characteristics.mana_cost,
        ManaCost {
            generic: 1.into(),
            colored: vec![Color::B]
        }
    );
    assert_eq!(artist.characteristics.power, Some(0.into()));
    assert_eq!(artist.characteristics.toughness, Some(1.into()));
    assert_eq!(
        artist.triggered[0].condition,
        TriggerCondition::AnyCreatureDies
    );
    assert_eq!(
        artist.triggered[0].program.targets,
        [TargetSpec {
            slot: 0,
            selection_id: "line:0:8".into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            filter: None,
            domain: TargetDomain::AnyPlayer
        }]
    );
    assert_eq!(
        artist.triggered[0].program.instructions,
        [
            Instruction::act(
                Verb::LoseLife {
                    player: PlayerExpr::target(0),
                    amount: Amount::fixed(1)
                },
                0
            ),
            Instruction::act(
                Verb::GainLife {
                    player: PlayerExpr::you(Context::Triggered),
                    amount: Amount::fixed(1)
                },
                0
            )
        ]
    );
    for d in [artist, warden] {
        let raw = fixtures::TRIGGER_RECORDS
            .iter()
            .find(|(_, raw)| {
                serde_json::from_str::<serde_json::Value>(raw).unwrap()["oracleIdentity"]
                    == d.oracle_id
            })
            .unwrap()
            .1;
        let record: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert_eq!(record["binding"]["fullyBound"], false);
    }
    for index in [22, 25] {
        assert!(matches!(
            compile(fixtures::MANIFEST, fixtures::TRIGGER_RECORDS[index].1),
            CompileResult::Unsupported { .. }
        ));
    }
}

#[test]
fn trigger_binder_canonicalizes_only_the_exact_disjoint_dies_partition() {
    let reversed = changed_source(fixtures::TRIGGER_RECORDS[11].1, |r| {
        r["faces"][0]["result"]["abilities"][0]["trigger"]["events"]
            .as_array_mut()
            .unwrap()
            .reverse();
        r["faces"][0]["result"]["lines"][0]["ability"] =
            r["faces"][0]["result"]["abilities"][0].clone();
    });
    let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &reversed) else {
        panic!("order of disjoint event alternatives must not change their meaning")
    };
    assert_eq!(
        definition.triggered[0].condition,
        TriggerCondition::AnyCreatureDies
    );
    let diagnostic = changed_source(fixtures::TRIGGER_RECORDS[23].1, |r| {
        r["binding"]["fullyBound"] = false.into()
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &diagnostic),
        CompileResult::Lowered { .. }
    ));
    let renamed = changed_source(fixtures::TRIGGER_RECORDS[24].1, |r| {
        r["source"]["name"] = "Renamed complete event source".into();
        r["faces"][0]["result"]["name"] = "Renamed complete event source".into();
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &renamed),
        CompileResult::Lowered { .. }
    ));
}

#[test]
fn trigger_lowering_rejects_unbound_alternatives_optional_state_mana_and_deferred_scopes() {
    type Mutation = (&'static str, usize, fn(&mut serde_json::Value));
    let mutations: Vec<Mutation> = vec![
        ("missing self alternative", 11, |r| {
            r["faces"][0]["result"]["abilities"][0]["trigger"]["events"]
                .as_array_mut()
                .unwrap()
                .remove(0);
        }),
        ("duplicate self alternative", 11, |r| {
            r["faces"][0]["result"]["abilities"][0]["trigger"]["events"][1] =
                r["faces"][0]["result"]["abilities"][0]["trigger"]["events"][0].clone()
        }),
        ("different event", 11, |r| {
            r["faces"][0]["result"]["abilities"][0]["trigger"]["events"][1]["trigger"] =
                "enters".into()
        }),
        ("unknown event argument", 23, |r| {
            r["faces"][0]["result"]["abilities"][0]["trigger"]["from"] = "graveyard".into()
        }),
        ("unbound other subject", 24, |r| {
            r["faces"][0]["result"]["abilities"][0]["trigger"]["what"]["filter"]["other"] =
                false.into()
        }),
        ("narrowed controller", 24, |r| {
            r["faces"][0]["result"]["abilities"][0]["trigger"]["what"]["filter"]["control"] =
                "you".into()
        }),
        ("different subject type", 24, |r| {
            r["faces"][0]["result"]["abilities"][0]["trigger"]["what"]["filter"]["types"] =
                serde_json::json!(["artifact"])
        }),
        ("nonunit event cardinality", 24, |r| {
            r["faces"][0]["result"]["abilities"][0]["trigger"]["what"]["count"]["value"] = 2.into()
        }),
        ("unbound effect pronoun", 11, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["who"]["player"] =
                "that-player".into()
        }),
        ("unsupported optional draw-two instruction", 23, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["optional"] =
                true.into();
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["amount"]["value"] =
                2.into();
        }),
        ("optional trigger", 23, |r| {
            r["faces"][0]["result"]["abilities"][0]["optional"] = true.into()
        }),
        ("state trigger", 23, |r| {
            r["faces"][0]["result"]["abilities"][0]["trigger"] = serde_json::json!({"trigger":"state","condition":{"condition":"life-total","value":0}})
        }),
        ("mana trigger", 23, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0] = serde_json::json!({"effect":"add-mana","who":{"player":"you"},"mana":{"mana":"fixed","symbols":["G"]}})
        }),
        ("intervening if", 23, |r| {
            r["faces"][0]["result"]["abilities"][0]["interveningIf"] =
                serde_json::json!({"condition":"life-total","value":0})
        }),
        ("conditional effect", 23, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["conditionKind"] = "if".into()
        }),
        ("delayed trigger", 23, |r| {
            r["faces"][0]["result"]["abilities"][0]["kind"] = "delayed-triggered".into()
        }),
        ("reflexive trigger", 23, |r| {
            r["faces"][0]["result"]["abilities"][0]["kind"] = "reflexive-triggered".into()
        }),
    ];
    for (label, index, change) in mutations {
        let raw = changed_source(fixtures::TRIGGER_RECORDS[index].1, |r| {
            change(r);
            r["binding"]["fullyBound"] = true.into();
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        let result = compile(fixtures::MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{label}: {result:?}"
        );
    }
    let extra = changed_source(fixtures::TRIGGER_RECORDS[23].1, |r| {
        let original = r["source"]["oracle_text"].as_str().unwrap();
        r["source"]["oracle_text"] = format!("{original}\nProtection from red").into();
        let ability = serde_json::json!({"kind":"keywords","keywords":[{"keyword":"protection"}]});
        r["faces"][0]["result"]["abilities"]
            .as_array_mut()
            .unwrap()
            .push(ability.clone());
        r["faces"][0]["result"]["lines"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"text":"Protection from red","ok":true,"ability":ability}));
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &extra),
        CompileResult::Unsupported { .. }
    ));
}

#[test]
fn triggered_ir_requires_exact_capability_closure_and_mandatory_stack_programs() {
    let c = embedded_catalog("trigger").unwrap();
    for change in [
        (|d: &mut Definition| d.triggered[0].functioning_zone = FunctioningZone::Stack)
            as fn(&mut Definition),
        |d| d.triggered[0].program.functioning_zone = FunctioningZone::Battlefield,
        |d| d.triggered[0].program.timing = SpellTiming::MainPhase,
        |d| d.triggered[0].program.resolution = SpellResolution::Permanent,
        |d| d.triggered[0].program.targets[0].domain = TargetDomain::OpponentPlayer,
        |d| {
            d.triggered[0].program.instructions[0] = Instruction::act(
                Verb::LoseLife {
                    player: PlayerExpr::target(1),
                    amount: Amount::fixed(1),
                },
                0,
            )
        },
        |d| {
            d.capabilities.remove(&Capability::CreatureDiesV1);
        },
    ] {
        let mut bad = c.clone();
        change(
            bad.definitions
                .iter_mut()
                .find(|d| d.name == "Blood Artist")
                .unwrap(),
        );
        assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
    }
    let raw = canonical(&c).unwrap();
    for bad in [
        raw.replace("any_creature_dies", "other_creature_dies"),
        raw.replace(
            "\"condition\":\"any_creature_dies\"",
            "\"condition\":{\"any_creature_dies\":{\"extra\":true}}",
        ),
        raw.replace(
            "\"condition\":\"any_creature_dies\"",
            "\"optional\":true,\"condition\":\"any_creature_dies\"",
        ),
    ] {
        assert_ne!(bad, raw);
        assert!(Catalog::read(&bad).is_err());
    }
    let mut bad = c;
    bad.schema_version = mtg_contracts::CURRENT_IR_VERSION + 1;
    assert!(bad.validate().is_err());
}

#[test]
fn damage_life_pool_retains_all_source_blocks_targets_quantities_and_printed_costs() {
    let c = embedded_catalog("damage-life").unwrap();
    assert_eq!(c.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-damage-life-ir.json"
        ))
        .unwrap(),
        c
    );
    let get = |name: &str| c.definitions.iter().find(|d| d.name == name).unwrap();
    let bolt = get("Lightning Bolt");
    let p = bolt.spell.as_ref().unwrap();
    assert_eq!(
        bolt.characteristics.mana_cost,
        ManaCost {
            generic: 0.into(),
            colored: vec![Color::R]
        }
    );
    assert_eq!(p.targets[0].domain, TargetDomain::AnyDamageTarget);
    assert_eq!(p.targets[0].selection_id, "line:0:6");
    assert_eq!(
        p.instructions,
        [Instruction::damage_target(
            0,
            kir::Amount::Fixed { value: 3.into() },
            0
        )]
    );
    let sorcerer = get("Prodigal Sorcerer");
    let a = &sorcerer.activated[0];
    assert_eq!(
        sorcerer.characteristics.mana_cost,
        ManaCost {
            generic: 2.into(),
            colored: vec![Color::U]
        }
    );
    assert_eq!(sorcerer.characteristics.power, Some(1.into()));
    assert_eq!(sorcerer.characteristics.toughness, Some(1.into()));
    assert_eq!(a.cost, ActivationCost::TapSource);
    assert_eq!(a.program.timing, SpellTiming::Instant);
    assert_eq!(a.program.targets[0].domain, TargetDomain::AnyDamageTarget);
    assert_eq!(a.program.targets[0].selection_id, "line:0:9");
    assert_eq!(
        a.program.instructions,
        [Instruction::damage_target(
            0,
            kir::Amount::Fixed { value: 1.into() },
            0
        )]
    );
    let soul = get("Soul Feast");
    let p = soul.spell.as_ref().unwrap();
    assert_eq!(
        soul.characteristics.mana_cost,
        ManaCost {
            generic: 3.into(),
            colored: vec![Color::B, Color::B]
        }
    );
    assert_eq!(p.targets[0].domain, TargetDomain::AnyPlayer);
    assert_eq!(
        p.instructions,
        [
            Instruction::act(
                Verb::LoseLife {
                    player: PlayerExpr::target(0),
                    amount: Amount::fixed(4)
                },
                0
            ),
            Instruction::act(
                Verb::GainLife {
                    player: PlayerExpr::you(Context::Spell),
                    amount: Amount::fixed(4)
                },
                0
            )
        ]
    );
    let hands = get("Healing Hands");
    let p = hands.spell.as_ref().unwrap();
    assert_eq!(
        hands.characteristics.mana_cost,
        ManaCost {
            generic: 2.into(),
            colored: vec![Color::W]
        }
    );
    assert_eq!(
        p.instructions,
        [
            Instruction::act(
                Verb::GainLife {
                    player: PlayerExpr::target(0),
                    amount: Amount::fixed(4)
                },
                0
            ),
            Instruction::draw(PlayerExpr::you(Context::Spell), 1.into(), 1)
        ]
    );
    let nectar = get("Sacred Nectar");
    let p = nectar.spell.as_ref().unwrap();
    assert_eq!(
        nectar.characteristics.mana_cost,
        ManaCost {
            generic: 1.into(),
            colored: vec![Color::W]
        }
    );
    assert!(p.targets.is_empty());
    assert_eq!(p.timing, SpellTiming::MainPhase);
    assert_eq!(
        p.instructions,
        [Instruction::act(
            Verb::GainLife {
                player: PlayerExpr::you(Context::Spell),
                amount: Amount::fixed(4)
            },
            0
        )]
    );
    let mountain = get("Mountain");
    assert_eq!(mountain.characteristics.subtypes, ["Mountain"]);
    assert_eq!(mountain.characteristics.color_identity, [Color::R]);
    assert_eq!(basic_land_color("Mountain"), Some(Color::R));
    assert!(matches!(
        compile(fixtures::MANIFEST, fixtures::DAMAGE_LIFE_RECORDS[22].1),
        CompileResult::Unsupported { .. }
    ));
}

#[test]
fn complete_damage_life_sources_reject_unknown_scopes_sources_amounts_and_missing_blocks() {
    type Mutation = (usize, fn(&mut serde_json::Value));
    let mutations: Vec<Mutation> = vec![
        (16, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["source"] =
                serde_json::json!({"ref":"this","noun":"creature"})
        }),
        (16, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["source"]["extra"] =
                true.into()
        }),
        (16, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["to"]["controller"] =
                "you".into()
        }),
        (16, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["amount"]["amount"] =
                "x".into()
        }),
        (16, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["amount"]["value"] =
                0.into()
        }),
        (16, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["to"]["selectionId"] =
                "".into()
        }),
        (17, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["source"] =
                serde_json::json!({"ref":"self"})
        }),
        (17, |r| {
            r["faces"][0]["result"]["abilities"][0]["costs"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"cost":"mana","symbols":["U"]}))
        }),
        (18, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["who"]["player"] =
                "target-opponent".into()
        }),
        (18, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][1]["who"]["selectionId"] =
                "fake-controller-target".into()
        }),
        (19, |r| {
            r["faces"][0]["result"]["abilities"][1]["effects"][0]["steps"][0]["amount"]["value"] =
                3.into()
        }),
        (20, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["who"]["player"] =
                "each-player".into()
        }),
    ];
    for (index, change) in mutations {
        let raw = changed_source(fixtures::DAMAGE_LIFE_RECORDS[index].1, |r| {
            change(r);
            for block in 0..r["faces"][0]["result"]["abilities"]
                .as_array()
                .unwrap()
                .len()
            {
                r["faces"][0]["result"]["lines"][block]["ability"] =
                    r["faces"][0]["result"]["abilities"][block].clone();
            }
        });
        let result = compile(fixtures::MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "index {index}: {result:?}"
        );
    }
    let raw = changed_source(fixtures::DAMAGE_LIFE_RECORDS[19].1, |r| {
        r["faces"][0]["result"]["abilities"]
            .as_array_mut()
            .unwrap()
            .pop();
        r["faces"][0]["result"]["lines"]
            .as_array_mut()
            .unwrap()
            .pop();
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &raw),
        CompileResult::Invalid { .. }
    ));
    let raw = changed_source(fixtures::DAMAGE_LIFE_RECORDS[16].1, |r| {
        r["source"]["name"] = "Renamed damage source".into();
        r["faces"][0]["result"]["name"] = "Renamed damage source".into();
        r["source"]["oracle_text"] = "Renamed damage source deals 3 damage to any target.".into();
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &raw),
        CompileResult::Lowered { .. }
    ));
}

#[test]
fn damage_life_programs_require_strict_target_recipient_quantity_closure() {
    let catalog = embedded_catalog("damage-life").unwrap();
    let mut bad = catalog.clone();
    bad.schema_version = mtg_contracts::CURRENT_IR_VERSION + 1;
    assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
    for change in [
        (|d: &mut Definition| {
            d.spell.as_mut().unwrap().targets[0].domain = TargetDomain::BattlefieldCreature
        }) as fn(&mut Definition),
        |d| {
            d.spell.as_mut().unwrap().instructions[0] =
                Instruction::damage_target(1, kir::Amount::Fixed { value: 3.into() }, 0)
        },
        |d| {
            d.spell.as_mut().unwrap().instructions[0] = Instruction::damage_target(
                0,
                kir::Amount::Fixed {
                    value: Quantity::zero(),
                },
                0,
            )
        },
        |d| {
            d.capabilities.remove(&Capability::NoncombatDamageV1);
        },
    ] {
        let mut bad = catalog.clone();
        change(
            bad.definitions
                .iter_mut()
                .find(|d| d.name == "Lightning Bolt")
                .unwrap(),
        );
        assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
    }
    for change in [
        (|d: &mut Definition| {
            d.spell.as_mut().unwrap().instructions[0] = Instruction::act(
                Verb::LoseLife {
                    player: PlayerExpr::you(Context::Spell),
                    amount: Amount::fixed(4),
                },
                0,
            )
        }) as fn(&mut Definition),
        |d| {
            d.spell.as_mut().unwrap().instructions[0] = Instruction::act(
                Verb::LoseLife {
                    player: PlayerExpr::target(99),
                    amount: Amount::fixed(4),
                },
                0,
            )
        },
        |d| {
            d.spell.as_mut().unwrap().instructions[0] = Instruction::act(
                Verb::LoseLife {
                    player: PlayerExpr::target(0),
                    amount: Amount::fixed(-4),
                },
                0,
            )
        },
    ] {
        let mut bad = catalog.clone();
        let d = bad
            .definitions
            .iter_mut()
            .find(|d| d.name == "Soul Feast")
            .unwrap();
        change(d);
        d.capabilities = d.requirements();
        assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
    }
    let raw = canonical(&catalog).unwrap();
    for (from, to) in [
        ("any_damage_target", "creature_or_player"),
        ("any_player", "opponent_only"),
        // CR 109.5: a spell's "you" is never a static ability's controller.
        (
            "\"role\":\"spell_controller\"",
            "\"role\":\"static_controller\"",
        ),
    ] {
        let bad = raw.replace(from, to);
        assert_ne!(bad, raw);
        assert!(Catalog::read(&bad).is_err());
    }
}

#[test]
fn mixed_spell_programs_and_activation_shapes_are_checked() {
    let mut c = embedded_catalog("base").unwrap();
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Unsummon")
        .unwrap();
    d.spell
        .as_mut()
        .unwrap()
        .instructions
        .push(Instruction::draw(
            PlayerExpr::you(Context::Spell),
            1.into(),
            1,
        ));
    d.capabilities = d.requirements();
    c.validate().unwrap();
    let mut c = embedded_catalog("damage-life").unwrap();
    let d = c
        .definitions
        .iter_mut()
        .find(|d| d.name == "Prodigal Sorcerer")
        .unwrap();
    d.activated[0].program.instructions = vec![Instruction::draw(
        PlayerExpr::you(Context::Activated),
        1.into(),
        0,
    )];
    d.capabilities = d.requirements();
    assert!(c.validate().is_err());
}

#[test]
fn stack_pool_retains_complete_counter_domains_and_costs() {
    let catalog = embedded_catalog("stack").unwrap();
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
    assert_eq!(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-stack-ir.json"
        ))
        .unwrap(),
        catalog
    );
    for (name, domain, colored) in [
        (
            "Counterspell",
            TargetDomain::StackSpell,
            vec![Color::U, Color::U],
        ),
        (
            "Stifle",
            TargetDomain::StackActivatedOrTriggeredAbility,
            vec![Color::U],
        ),
    ] {
        let d = catalog.definitions.iter().find(|d| d.name == name).unwrap();
        assert_eq!(
            d.characteristics.mana_cost,
            ManaCost {
                generic: Quantity::zero(),
                colored
            }
        );
        let p = d.spell.as_ref().unwrap();
        assert_eq!(p.targets.len(), 1);
        assert_eq!(p.targets[0].domain, domain);
        assert_eq!(
            p.instructions,
            [Instruction::CounterStackObject {
                target_slot: 0,
                source_block: 0
            }]
        );
    }
    let looter = catalog
        .definitions
        .iter()
        .find(|d| d.name == "Merfolk Looter")
        .unwrap();
    assert_eq!(looter.activated[0].cost, ActivationCost::TapSource);
    assert_eq!(
        looter.activated[0].program.instructions,
        [
            Instruction::draw(PlayerExpr::you(Context::Activated), Quantity::from(1), 0),
            Instruction::discard(
                kir::PlayerExpr::you(kir::Context::Activated),
                kir::Amount::Fixed {
                    value: Quantity::from(1)
                },
                false,
                0
            ),
        ]
    );
    let mage = catalog
        .definitions
        .iter()
        .find(|d| d.name == "Azure Mage")
        .unwrap();
    assert_eq!(
        mage.activated[0].cost,
        ActivationCost::Mana {
            cost: ManaCost {
                generic: Quantity::from(3),
                colored: vec![Color::U]
            }
        }
    );
    assert_eq!(
        mage.activated[0].program.instructions,
        [Instruction::draw(
            PlayerExpr::you(Context::Activated),
            Quantity::from(1),
            0
        )]
    );
    for d in [looter, mage] {
        assert_eq!(
            d.characteristics.mana_cost,
            ManaCost {
                generic: Quantity::from(1),
                colored: vec![Color::U]
            }
        );
        assert_eq!(d.activated.len(), 1);
    }
}

#[test]
fn complete_stack_sources_reject_narrowed_filters_combined_costs_and_extra_effects() {
    type Mutation = (usize, fn(&mut serde_json::Value));
    let mutations: Vec<Mutation> = vec![
        (12, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]["controller"] =
                "you".into()
        }),
        (12, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["count"]["value"] =
                2.into()
        }),
        (13, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]["anyOf"] =
                serde_json::json!([{"abilityKind":"activated"}])
        }),
        (13, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["filter"]["anyOf"]
                [1]["abilityKind"] = "mana".into()
        }),
        (13, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["selectionId"] =
                "".into()
        }),
        (14, |r| {
            r["faces"][0]["result"]["abilities"][0]["costs"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"cost":"mana","symbols":["U"]}))
        }),
        (14, |r| {
            r["faces"][0]["result"]["abilities"][0]["costs"][0]["what"] =
                serde_json::json!({"ref":"another","noun":"creature"})
        }),
        (15, |r| {
            r["faces"][0]["result"]["abilities"][0]["costs"][0]["symbols"] =
                serde_json::json!(["X", "U"])
        }),
        (15, |r| {
            r["faces"][0]["result"]["abilities"][0]["costs"][0]["symbols"] =
                serde_json::json!(["3}{U"])
        }),
        (15, |r| {
            r["faces"][0]["result"]["abilities"][0]["costs"][0]["symbols"] = serde_json::json!([])
        }),
        (15, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"].as_array_mut().unwrap().push(serde_json::json!({"effect":"gain-life","amount":{"amount":"fixed","value":1},"who":{"player":"you"}}))
        }),
    ];
    for (index, change) in mutations {
        let raw = changed_source(fixtures::STACK_RECORDS[index].1, |r| {
            change(r);
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        let result = compile(fixtures::MANIFEST, &raw);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "index {index}: {result:?}"
        );
    }
    for (_, source) in &fixtures::STACK_RECORDS[12..] {
        let raw = changed_source(source, |r| {
            r["source"]["name"] = "Renamed complete source".into();
            r["faces"][0]["result"]["name"] = "Renamed complete source".into();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &raw),
            CompileResult::Lowered { .. }
        ));
        let raw = changed_source(source, |r| {
            let original = r["source"]["oracle_text"].as_str().unwrap();
            r["source"]["oracle_text"] = format!("{original}\nMenace").into();
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &raw),
            CompileResult::Invalid { .. }
        ));
    }
}

#[test]
fn stack_vocabulary_rejects_bad_binding_and_capability_closure() {
    let catalog = embedded_catalog("stack").unwrap();
    for change in [
        (|d: &mut Definition| {
            d.spell.as_mut().unwrap().targets[0].domain = TargetDomain::BattlefieldCreature
        }) as fn(&mut Definition),
        |d| {
            d.spell.as_mut().unwrap().instructions[0] = Instruction::CounterStackObject {
                target_slot: 1,
                source_block: 0,
            }
        },
        |d| {
            d.capabilities.remove(&Capability::CounterStackObjectV1);
        },
    ] {
        let mut bad = catalog.clone();
        let d = bad
            .definitions
            .iter_mut()
            .find(|d| d.name == "Counterspell")
            .unwrap();
        change(d);
        assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
    }
    let mut bad = catalog.clone();
    let d = bad
        .definitions
        .iter_mut()
        .find(|d| d.name == "Azure Mage")
        .unwrap();
    d.activated[0].cost = ActivationCost::Mana {
        cost: ManaCost {
            generic: Quantity::from(-1),
            colored: vec![],
        },
    };
    assert!(Catalog::read(&canonical(&bad).unwrap()).is_err());
    let raw = canonical(&catalog).unwrap();
    for bad in [
        raw.replace(
            "stack_activated_or_triggered_ability",
            "stack_activated_ability",
        ),
        raw.replace("\"cost\":{\"mana\":", "\"cost\":{\"unknown\":"),
        raw.replace("\"tap_source\"", "{\"tap_source\":{\"extra\":true}}"),
    ] {
        assert!(Catalog::read(&bad).is_err());
    }
}

#[test]
fn rehashed_unsupported_semantics_are_not_hidden_by_hash_rejection() {
    type Mutation = (usize, fn(&mut serde_json::Value));
    let mutations: Vec<Mutation> = vec![
        (3, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["unknown"] =
                true.into();
        }),
        (3, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["what"]["selectionId"] =
                "".into();
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        }),
        (3, |r| {
            r["source"]["layout"] = "transform".into();
        }),
        (3, |r| {
            r["faces"][0]["result"]["abilities"] =
                serde_json::json!([{"kind":"keywords","keywords":[{"keyword":"protection"}]}]);
        }),
        (1, |r| {
            r["source"]["toughness"] = "*".into();
        }),
        (1, |r| {
            r["source"].as_object_mut().unwrap().remove("power");
        }),
        (2, |r| {
            r["source"]["oracle_text"] = "Menace".into();
        }),
        (3, |r| {
            r["source"]["mana_cost"] = "{X}{U}".into();
        }),
        (3, |r| {
            r["source"]["oracle_text"] =
                "Return target creature to its owner's hand.\nDraw a card.".into();
        }),
    ];
    for (index, mutation) in mutations {
        let result = compile(fixtures::MANIFEST, &changed_record(index, mutation));
        assert!(
            !matches!(result, CompileResult::Lowered { .. }),
            "unexpected lowering: {result:?}"
        );
    }
}
#[test]
fn external_metadata_and_display_names_do_not_supply_semantics() {
    let r = changed_record(3, |r| {
        r["source"]["unconsumed_metadata"] = serde_json::json!({"unicode":"Δ","number":1.5});
        r["source"]["name"] = "Renamed source fixture".into();
        r["faces"][0]["result"]["name"] = "Renamed source fixture".into();
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &r),
        CompileResult::Lowered { .. }
    ));
}
#[test]
fn lowers_intrinsics_and_complete_spell_without_name_dispatch() {
    let c = embedded_catalog("base").unwrap();
    let island = c.definitions.iter().find(|d| d.name == "Island").unwrap();
    assert!(island.spell.is_none());
    assert_eq!(island.intrinsic_zone, Some(FunctioningZone::Battlefield));
    let spell = c
        .definitions
        .iter()
        .find(|d| d.name == "Unsummon")
        .unwrap()
        .spell
        .as_ref()
        .unwrap();
    assert_eq!(spell.targets[0].selection_id, "line:0:1");
    assert_eq!(
        spell.instructions,
        [Instruction::move_target(0, kir::Destination::OwnerHand, 0)]
    );
    assert_eq!(Catalog::read(&canonical(&c).unwrap()).unwrap(), c);
}
#[test]
fn opponent_sacrifice_lowers_as_resolution_choice() {
    let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, fixtures::RECORDS[5].1)
    else {
        panic!("complete opponent sacrifice source must lower")
    };
    let program = definition.spell.as_ref().unwrap();
    assert_eq!(definition.characteristics.kind, CardKind::Sorcery);
    assert_eq!(program.timing, SpellTiming::MainPhase);
    assert_eq!(
        program.targets,
        [TargetSpec {
            slot: 0,
            selection_id: "line:0:0".into(),
            chooser: Chooser::Controller,
            stage: EvaluationStage::Announcement,
            filter: None,
            domain: TargetDomain::OpponentPlayer
        }]
    );
    assert_eq!(
        program.instructions,
        [Instruction::sacrifice_creature(
            kir::PlayerExpr::target(0),
            0
        )]
    );
    assert!(definition.activated.is_empty());
}
#[test]
fn rejects_hash_version_unknown_field_and_incomplete_face() {
    let raw = fixtures::RECORDS[3].1;
    for bad in [
        raw.replace("\"status\":\"parsed\"", "\"status\":\"unsupported\""),
        raw.replacen(
            "\"sourceIndex\":",
            "\"extraSemanticField\":true,\"sourceIndex\":",
            1,
        ),
        raw.replace("Return target creature", "Destroy target creature"),
        raw.replace("\"sourceFaceIndex\":0", "\"sourceFaceIndex\":1"),
    ] {
        assert!(!matches!(
            compile(fixtures::MANIFEST, &bad),
            CompileResult::Lowered { .. }
        ));
    }
    let bad = fixtures::MANIFEST.replace("\"schemaVersion\":3", "\"schemaVersion\":4");
    assert!(matches!(compile(&bad, raw), CompileResult::Invalid { .. }));
}
#[test]
fn semantic_ir_is_strict_and_capability_checked() {
    let c = embedded_catalog("base").unwrap();
    let raw = canonical(&c).unwrap();
    for bad in [
        raw.replace(
            "\"schema_version\":1",
            &format!(
                "\"schema_version\":{}",
                mtg_contracts::CURRENT_IR_VERSION + 1
            ),
        ),
        raw.replace("{\"to\":\"owner_hand\"}", "{\"to\":\"execute_text\"}"),
        raw.replace("\"slot\":0", "\"slot\":99"),
        raw.replace("\"source_block\":0", "\"unknown\":true,\"source_block\":0"),
        raw.replace(
            "\"functioning_zone\":\"stack\"",
            "\"functioning_zone\":\"battlefield\"",
        ),
    ] {
        assert!(Catalog::read(&bad).is_err());
    }
}
#[test]
fn vanilla_creature_spells_and_land_permissions_are_explicit() {
    let catalog = embedded_catalog("base").unwrap();
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
    for d in &catalog.definitions {
        match d.characteristics.kind {
            CardKind::Land => {
                assert!(d.spell.is_none());
                assert_eq!(
                    d.capabilities,
                    BTreeSet::from([
                        Capability::NormalLayoutV1,
                        Capability::BasicLandManaV1,
                        Capability::LandPlayV1,
                    ])
                );
            }
            CardKind::Creature => {
                let spell = d.spell.as_ref().unwrap();
                assert_eq!(spell.timing, SpellTiming::MainPhase);
                assert_eq!(spell.resolution, SpellResolution::Permanent);
                assert_eq!(spell.functioning_zone, FunctioningZone::Stack);
                assert!(spell.targets.is_empty());
                assert!(spell.instructions.is_empty());
                assert!(d.capabilities.contains(&Capability::CastCreatureV1));
                assert_eq!(
                    d.capabilities.contains(&Capability::CommanderIdentityV1),
                    d.characteristics.legendary
                );
            }
            CardKind::Instant => {
                let spell = d.spell.as_ref().unwrap();
                assert_eq!(spell.timing, SpellTiming::Instant);
                assert_eq!(spell.resolution, SpellResolution::Instructions);
                assert!(!spell.instructions.is_empty());
            }
            CardKind::Sorcery | CardKind::Enchantment | CardKind::Artifact => {
                panic!("base fixture contains an unexpected card kind")
            }
        }
        for capability in &d.capabilities {
            let mut missing = d.clone();
            missing.capabilities.remove(capability);
            assert!(missing.validate().is_err(), "missing {capability:?}");
        }
    }
    // Same base semantics on a different source identity/display name must
    // lower; runtime exact-profile admission remains a separate check.
    let renamed = changed_record(1, |r| {
        r["source"]["name"] = "Renamed vanilla source".into();
        r["faces"][0]["result"]["name"] = "Renamed vanilla source".into();
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &renamed),
        CompileResult::Lowered { .. }
    ));
}

#[test]
fn commander_pool_adds_only_exact_source_backed_white_mana() {
    let catalog = embedded_catalog("commander").unwrap();
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
    let plains = catalog
        .definitions
        .iter()
        .find(|definition| definition.oracle_id == "bc71ebf6-2056-41f7-be35-b2e5c34afa99")
        .unwrap();
    assert_eq!(plains.name, "Plains");
    assert_eq!(plains.provenance.source_index, 28278);
    assert_eq!(plains.characteristics.kind, CardKind::Land);
    assert_eq!(plains.characteristics.subtypes, ["Plains"]);
    assert!(plains.characteristics.basic);
    assert_eq!(plains.characteristics.colors, []);
    assert_eq!(plains.characteristics.color_identity, [Color::W]);
    assert_eq!(plains.characteristics.mana_cost.generic, Quantity::zero());
    assert!(plains.characteristics.mana_cost.colored.is_empty());
    assert_eq!(
        basic_land_color(&plains.characteristics.subtypes[0]),
        Some(Color::W)
    );
    assert_eq!(plains.intrinsic_zone, Some(FunctioningZone::Battlefield));
    assert!(plains.spell.is_none());
    assert_eq!(plains.provenance.abilities_json, "[]");

    let commander = catalog
        .definitions
        .iter()
        .find(|definition| definition.oracle_id == "cfb756dd-6fe1-430a-8f04-8b10cac62a86")
        .unwrap();
    assert!(commander.characteristics.legendary);
    assert_eq!(
        commander.characteristics.mana_cost.generic,
        Quantity::zero()
    );
    assert_eq!(commander.characteristics.mana_cost.colored, [Color::W]);
    assert_eq!(commander.characteristics.color_identity, [Color::W]);
    assert!(
        commander
            .capabilities
            .contains(&Capability::CommanderIdentityV1)
    );
    // Command-zone permission and the additional cost belong to game rules:
    // they do not rewrite source mana cost or the card's complete program.
    assert_eq!(
        commander.spell.as_ref().unwrap().timing,
        SpellTiming::MainPhase
    );
    assert_eq!(
        catalog,
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-commander-ir.json"
        ))
        .unwrap()
    );
    assert_eq!(
        Catalog::read(&canonical(&catalog).unwrap()).unwrap(),
        catalog
    );
}

#[test]
fn white_mana_fixture_does_not_admit_additional_land_semantics() {
    type Mutation = fn(&mut serde_json::Value);
    let mutations: Vec<Mutation> = vec![
        |record| record["source"]["type_line"] = "Land — Plains Desert".into(),
        |record| record["source"]["oracle_text"] = "{T}: Add {W}.".into(),
        |record| record["source"]["oracle_text"] = "When this land enters, draw a card.".into(),
        |record| record["source"]["card_faces"] = serde_json::json!([{}]),
    ];
    for mutation in mutations {
        let result = compile(fixtures::MANIFEST, &changed_record(6, mutation));
        assert!(
            !matches!(result, CompileResult::Lowered { .. }),
            "unsupported land semantics disappeared: {result:?}"
        );
    }
    for line in ["Land — Plains Island", "Legendary Land — Plains"] {
        let raw = changed_record(6, |r| r["source"]["type_line"] = line.into());
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &raw) else {
            panic!("supported pure intrinsic land: {line}");
        };
        assert!(
            definition
                .capabilities
                .contains(&Capability::TypedNonbasicIntrinsicManaV1)
        );
        assert!(definition.mana_abilities.is_empty());
        assert!(definition.static_abilities.is_empty());
        assert!(
            definition
                .characteristics
                .subtypes
                .iter()
                .all(|s| basic_land_color(s).is_some())
        );
    }
    let renamed = changed_record(6, |record| {
        record["source"]["name"] = "Renamed white basic source".into();
        record["faces"][0]["result"]["name"] = "Renamed white basic source".into();
    });
    let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &renamed) else {
        panic!("intrinsic mana must follow basic subtype rather than card name");
    };
    assert_eq!(
        basic_land_color(&definition.characteristics.subtypes[0]),
        Some(Color::W)
    );
}

#[test]
fn library_catalog_lowers_complete_source_with_explicit_top_and_sorcery_timing() {
    let catalog = embedded_catalog("library").unwrap();
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
    let ebb = catalog
        .definitions
        .iter()
        .find(|d| d.oracle_id == "30cc8f7b-3c28-40f5-8f8f-157e8212280b")
        .unwrap();
    assert_eq!(ebb.name, "Time Ebb");
    assert_eq!(ebb.provenance.source_index, 7282);
    assert_eq!(ebb.characteristics.kind, CardKind::Sorcery);
    assert_eq!(
        ebb.characteristics.mana_cost,
        ManaCost {
            generic: Quantity::from(2),
            colored: vec![Color::U]
        }
    );
    assert_eq!(ebb.characteristics.colors, [Color::U]);
    assert_eq!(ebb.characteristics.color_identity, [Color::U]);
    assert_eq!(
        ebb.spell,
        Some(SpellProgram {
            optional_additional_costs: vec![],
            additional_sacrifice: None,
            modes: vec![],
            timing: SpellTiming::MainPhase,
            resolution: SpellResolution::Instructions,
            functioning_zone: FunctioningZone::Stack,
            targets: vec![TargetSpec {
                slot: 0,
                selection_id: "line:0:1".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                filter: None,
                domain: TargetDomain::BattlefieldCreature
            }],
            instructions: vec![Instruction::move_target(
                0,
                kir::Destination::OwnerLibraryTop,
                0
            )],
        })
    );
    assert_eq!(
        ebb.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::CastSorceryV1,
            Capability::CreatureTargetV1,
            Capability::OwnerLibraryTopMoveV1
        ])
    );
    assert_eq!(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-library-ir.json"
        ))
        .unwrap(),
        catalog
    );
    assert_eq!(
        Catalog::read(&canonical(&catalog).unwrap()).unwrap(),
        catalog
    );
    for capability in &ebb.capabilities {
        let mut changed = ebb.clone();
        changed.capabilities.remove(capability);
        assert!(changed.validate().is_err());
    }
    let renamed = changed_record(7, |r| {
        r["source"]["name"] = "Renamed library-top sorcery".into();
        r["faces"][0]["result"]["name"] = "Renamed library-top sorcery".into();
    });
    let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &renamed) else {
        panic!("library movement cannot depend on display names")
    };
    assert_eq!(definition.spell, ebb.spell);
}

#[test]
fn damage_pool_is_exact_empty_text_legendary_without_new_ability_vocabulary() {
    let catalog = embedded_catalog("damage").unwrap();
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
    let commander = catalog
        .definitions
        .iter()
        .find(|d| d.oracle_id == "980e1721-3717-4425-95a4-938dbf2ac661")
        .unwrap();
    assert_eq!(commander.name, "Yargle and Multani");
    assert_eq!(commander.provenance.source_index, 22862);
    assert_eq!(commander.characteristics.kind, CardKind::Creature);
    assert!(commander.characteristics.legendary);
    assert_eq!(commander.characteristics.power, Some(Quantity::from(18)));
    assert_eq!(commander.characteristics.toughness, Some(Quantity::from(6)));
    assert_eq!(
        commander.characteristics.mana_cost,
        ManaCost {
            generic: Quantity::from(3),
            colored: vec![Color::B, Color::B, Color::G]
        }
    );
    assert_eq!(commander.characteristics.colors, [Color::B, Color::G]);
    assert_eq!(
        commander.characteristics.color_identity,
        [Color::B, Color::G]
    );
    assert_eq!(commander.provenance.abilities_json, "[]");
    assert_eq!(
        commander.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::BaseCreatureV1,
            Capability::CastCreatureV1,
            Capability::CommanderIdentityV1
        ])
    );
    let spell = commander.spell.as_ref().unwrap();
    assert_eq!(spell.resolution, SpellResolution::Permanent);
    assert!(spell.instructions.is_empty());
    assert_eq!(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-damage-ir.json"
        ))
        .unwrap(),
        catalog
    );
    assert_eq!(
        Catalog::read(&canonical(&catalog).unwrap()).unwrap(),
        catalog
    );
    for (text, ability) in [
        (
            "Ward {1}",
            serde_json::json!({"kind":"keywords","keywords":[{"keyword":"ward"}]}),
        ),
        (
            "When this creature attacks, draw a card.",
            serde_json::json!({"kind":"triggered","effects":[]}),
        ),
    ] {
        let bad = changed_source(fixtures::DAMAGE_RECORDS[7].1, |r| {
            r["source"]["oracle_text"] = text.into();
            r["faces"][0]["result"]["lines"] =
                serde_json::json!([{"text":text,"ok":true,"ability":ability}]);
            r["faces"][0]["result"]["abilities"] = serde_json::json!([ability]);
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &bad),
            CompileResult::Unsupported { .. }
        ));
    }
    for mutate in [
        (|r: &mut serde_json::Value| r["source"]["power"] = "*".into())
            as fn(&mut serde_json::Value),
        |r| r["source"]["toughness"] = "0".into(),
        |r| r["source"]["mana_cost"] = "{X}{B}{B}{G}".into(),
    ] {
        let bad = changed_source(fixtures::DAMAGE_RECORDS[7].1, mutate);
        assert!(matches!(
            compile(fixtures::MANIFEST, &bad),
            CompileResult::Unsupported { .. }
        ));
    }
}

#[test]
fn action_pool_retains_printed_costs_and_complete_distinct_activation_program() {
    let catalog = embedded_catalog("action").unwrap();
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);
    let researcher = catalog
        .definitions
        .iter()
        .find(|d| d.name == "Hapless Researcher")
        .unwrap();
    assert_eq!(researcher.provenance.source_index, 5423);
    assert_eq!(
        researcher.characteristics.mana_cost,
        ManaCost {
            generic: Quantity::zero(),
            colored: vec![Color::U]
        }
    );
    assert_eq!(researcher.characteristics.power, Some(Quantity::from(1)));
    assert_eq!(
        researcher.characteristics.toughness,
        Some(Quantity::from(1))
    );
    assert_eq!(
        researcher.activated,
        [ActivatedAbility {
            functioning_zone: FunctioningZone::Battlefield,
            cost: ActivationCost::SacrificeSource,
            program: SpellProgram {
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                modes: vec![],
                timing: SpellTiming::Instant,
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![
                    Instruction::draw(PlayerExpr::you(Context::Activated), Quantity::from(1), 0),
                    Instruction::discard(
                        kir::PlayerExpr::you(kir::Context::Activated),
                        kir::Amount::Fixed {
                            value: Quantity::from(1)
                        },
                        false,
                        0
                    )
                ]
            }
        }]
    );
    let creature_spell = researcher.spell.as_ref().unwrap();
    assert_eq!(creature_spell.resolution, SpellResolution::Permanent);
    assert!(creature_spell.instructions.is_empty());
    assert_eq!(
        researcher.capabilities,
        BTreeSet::from([
            Capability::NormalLayoutV1,
            Capability::BaseCreatureV1,
            Capability::CastCreatureV1,
            Capability::ActivatedAbilityV1,
            Capability::SacrificeSourceCostV1,
            Capability::ControllerDrawV1,
            Capability::ControllerDiscardV1
        ])
    );
    let edict = catalog
        .definitions
        .iter()
        .find(|d| d.name == "Cruel Edict")
        .unwrap();
    assert_eq!(
        edict.characteristics.mana_cost,
        ManaCost {
            generic: Quantity::from(1),
            colored: vec![Color::B]
        }
    );
    assert!(edict.capabilities.contains(&Capability::OpponentTargetV1));
    assert!(!edict.capabilities.contains(&Capability::CreatureTargetV1));
    let swamp = catalog
        .definitions
        .iter()
        .find(|d| d.name == "Swamp")
        .unwrap();
    assert_eq!(swamp.characteristics.subtypes, ["Swamp"]);
    assert_eq!(
        basic_land_color(&swamp.characteristics.subtypes[0]),
        Some(Color::B)
    );
    assert_eq!(swamp.provenance.source_index, 12962);
    assert_eq!(
        Catalog::read(include_str!(
            "../../../../tests/fixtures/expected-action-ir.json"
        ))
        .unwrap(),
        catalog
    );
    assert_eq!(
        Catalog::read(&canonical(&catalog).unwrap()).unwrap(),
        catalog
    );
    for (_, raw) in fixtures::ACTION_RECORDS {
        assert!(matches!(
            compile(fixtures::MANIFEST, raw),
            CompileResult::Lowered { .. }
        ));
    }
    for capability in &researcher.capabilities {
        let mut bad = researcher.clone();
        bad.capabilities.remove(capability);
        assert!(bad.validate().is_err());
    }
}

#[test]
fn optional_draw_single_card_variants_preserve_existing_entry_and_activation_sequences() {
    fn program(d: &Definition) -> &SpellProgram {
        if let Some(a) = d.activated.first() {
            &a.program
        } else {
            &d.triggered[0].program
        }
    }
    // Synthetic optional-one-card AST variants of Elvish Visionary and
    // Hapless Researcher. CR 603.5/608.2d and the optional draw instruction admit
    // this choice; it does not make the following discard conditional.
    for raw in [
        fixtures::TRIGGER_RECORDS[23].1,
        fixtures::ACTION_RECORDS[9].1,
    ] {
        let CompileResult::Lowered {
            definition: original,
        } = compile(fixtures::MANIFEST, raw)
        else {
            panic!("original archived source")
        };
        let optional = changed_source(raw, |r| {
            r["faces"][0]["result"]["abilities"][0]["effects"][0]["steps"][0]["optional"] =
                true.into();
            r["faces"][0]["result"]["lines"][0]["ability"] =
                r["faces"][0]["result"]["abilities"][0].clone();
        });
        let CompileResult::Lowered { definition } = compile(fixtures::MANIFEST, &optional) else {
            panic!("optional controller draw")
        };
        assert_eq!(
            program(&definition).instructions[0],
            Instruction::OptionalDrawCards { source_block: 0 }
        );
        assert_eq!(
            program(&definition).instructions[1..],
            program(&original).instructions[1..]
        );
        assert!(
            definition
                .capabilities
                .contains(&Capability::OptionalControllerDrawV1)
        );
    }
}

#[test]
fn complete_action_lowering_rejects_wrong_costs_choosers_cardinality_and_extra_clauses() {
    type Mutation = fn(&mut serde_json::Value);
    let activation_mutations: Vec<Mutation> = vec![
        |a| a["costs"][0]["cost"] = "tap".into(),
        |a| a["costs"][0]["what"]["ref"] = "target".into(),
        |a| a["costs"][0]["what"]["noun"] = "artifact".into(),
        |a| a["costs"] = serde_json::json!([]),
        |a| {
            let cost = a["costs"][0].clone();
            a["costs"].as_array_mut().unwrap().push(cost);
        },
        |a| a["effects"][0]["steps"][0]["who"]["player"] = "opponent".into(),
        |a| a["effects"][0]["steps"][0]["amount"]["value"] = 2.into(),
        |a| {
            // Optional controller draw supports exactly one card; optional draw-two
            // remains an unsupported complete shape.
            a["effects"][0]["steps"][0]["optional"] = true.into();
            a["effects"][0]["steps"][0]["amount"]["value"] = 2.into();
        },
        |a| a["effects"][0]["steps"][1]["what"]["filter"]["cls"] = "creature".into(),
        |a| a["effects"][0]["steps"][1]["what"]["count"]["value"] = 2.into(),
        |a| a["effects"][0]["steps"][1]["who"]["selectionId"] = "line:0:0".into(),
        |a| a["restrictions"] = serde_json::json!(["sorcery-speed"]),
    ];
    let edict_mutations: Vec<Mutation> = vec![
        |a| a["effects"][0]["steps"][0]["chosenBy"]["player"] = "you".into(),
        |a| a["effects"][0]["steps"][0]["chosenBy"]["selectionId"] = "different-opponent".into(),
        |a| a["effects"][0]["steps"][0]["who"]["player"] = "target-player".into(),
        |a| a["effects"][0]["steps"][0]["what"]["count"]["value"] = 2.into(),
        |a| {
            a["effects"][0]["steps"][0]["what"]["filter"]["types"] = serde_json::json!(["artifact"])
        },
        |a| a["effects"][0]["steps"][0]["optional"] = true.into(),
    ];
    for (raw, mutations) in [
        (fixtures::ACTION_RECORDS[9].1, activation_mutations),
        (fixtures::RECORDS[5].1, edict_mutations),
    ] {
        for mutate in mutations {
            let bad = changed_source(raw, |r| {
                let ability = &mut r["faces"][0]["result"]["abilities"][0];
                mutate(ability);
                let changed = ability.clone();
                r["faces"][0]["result"]["lines"][0]["ability"] = changed;
            });
            let result = compile(fixtures::MANIFEST, &bad);
            assert!(
                matches!(result, CompileResult::Unsupported { .. }),
                "{result:?}"
            );
        }
    }
    let bad = changed_source(fixtures::ACTION_RECORDS[9].1, |r| {
        r["source"]["oracle_text"] =
            "Sacrifice this creature: Draw a card, then discard a card. You gain 1 life.".into()
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &bad),
        CompileResult::Invalid { .. }
    ));
    let renamed = changed_source(fixtures::ACTION_RECORDS[9].1, |r| {
        r["source"]["name"] = "Renamed source-sacrifice looter".into();
        r["faces"][0]["result"]["name"] = "Renamed source-sacrifice looter".into();
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &renamed),
        CompileResult::Lowered { .. }
    ));
}

#[test]
fn action_ir_shapes_and_target_domains_are_strict() {
    let catalog = embedded_catalog("action").unwrap();
    let researcher = catalog
        .definitions
        .iter()
        .find(|d| !d.activated.is_empty())
        .unwrap();
    for mutate in [
        (|d: &mut Definition| d.activated[0].functioning_zone = FunctioningZone::Stack)
            as fn(&mut Definition),
        |d| d.activated[0].program.functioning_zone = FunctioningZone::Battlefield,
        |d| d.activated[0].program.timing = SpellTiming::MainPhase,
        |d| {
            d.activated[0].program.instructions = vec![Instruction::draw(
                PlayerExpr::you(Context::Activated),
                Quantity::from(2),
                0,
            )]
        },
        |d| {
            d.activated[0].program.targets = vec![TargetSpec {
                slot: 0,
                selection_id: "unexpected".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                filter: None,
                domain: TargetDomain::BattlefieldCreature,
            }]
        },
        |d| {
            d.activated[0].program.instructions =
                vec![Instruction::move_target(0, kir::Destination::OwnerHand, 0)]
        },
    ] {
        let mut bad = researcher.clone();
        mutate(&mut bad);
        bad.capabilities = bad.requirements();
        assert!(bad.validate().is_err());
    }
    let mut mixed = catalog
        .definitions
        .iter()
        .find(|d| d.name == "Cruel Edict")
        .unwrap()
        .clone();
    let p = mixed.spell.as_mut().unwrap();
    p.targets.push(TargetSpec {
        slot: 1,
        selection_id: "creature-slot".into(),
        chooser: Chooser::Controller,
        stage: EvaluationStage::Announcement,
        filter: None,
        domain: TargetDomain::BattlefieldCreature,
    });
    p.instructions
        .push(Instruction::move_target(1, kir::Destination::OwnerHand, 0));
    mixed.capabilities = mixed.requirements();
    assert!(mixed.validate().is_err());
    let raw = canonical(&catalog).unwrap();
    for bad in [
        raw.replace("\"cost\":\"sacrifice_source\"", "\"cost\":\"tap_source\""),
        raw.replace(
            "\"amount\":{\"amount\":\"fixed\",\"value\":\"1\"}",
            "\"amount\":{\"amount\":\"fixed\",\"value\":\"01\"}",
        ),
        raw.replace("\"verb\":\"draw\"", "\"verb\":\"draw\",\"optional\":true"),
    ] {
        assert!(Catalog::read(&bad).is_err());
    }
}

#[test]
fn complete_library_lowering_rejects_other_positions_owners_and_abilities() {
    type Mutation = fn(&mut serde_json::Value);
    let mutations: Vec<Mutation> = vec![
        |r| r["to"]["position"] = "bottom".into(),
        |r| r["to"]["position"] = "random".into(),
        |r| r["to"]["owner"] = "controller".into(),
        |r| r["to"]["zone"] = "hand".into(),
        |r| {
            r["to"].as_object_mut().unwrap().remove("position");
        },
        |r| r["to"]["shuffle"] = true.into(),
        |r| r["effect"] = "draw".into(),
        |r| r["optional"] = true.into(),
        |r| r["what"]["filter"]["types"] = serde_json::json!(["player"]),
        |r| r["what"]["count"]["value"] = 2.into(),
        |r| r["what"]["selectionId"] = "".into(),
    ];
    for mutate in mutations {
        let changed = changed_record(7, |record| {
            let ability = &mut record["faces"][0]["result"]["abilities"][0];
            mutate(&mut ability["effects"][0]["steps"][0]);
            let ability = ability.clone();
            record["faces"][0]["result"]["lines"][0]["ability"] = ability;
        });
        let result = compile(fixtures::MANIFEST, &changed);
        assert!(
            matches!(result, CompileResult::Unsupported { .. }),
            "{result:?}"
        );
    }
    let extra = changed_record(7, |r| {
        r["source"]["oracle_text"] =
            "Put target creature on top of its owner's library. Draw a card.".into();
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &extra),
        CompileResult::Invalid { .. }
    ));
}

#[test]
fn library_vocabulary_requires_valid_program_shape() {
    let catalog = embedded_catalog("library").unwrap();
    let raw = canonical(&catalog).unwrap();
    for bad in [
        raw.replace(
            "\"schema_version\":1",
            &format!(
                "\"schema_version\":{}",
                mtg_contracts::CURRENT_IR_VERSION + 1
            ),
        ),
        raw.replace(
            "{\"to\":\"owner_library_top\"}",
            "{\"to\":\"owner_library_bottom\"}",
        ),
        raw.replace(
            "{\"to\":\"owner_library_top\"}",
            "{\"to\":\"owner_library_top\",\"unknown\":true}",
        ),
    ] {
        assert!(Catalog::read(&bad).is_err());
    }
    let ebb = catalog
        .definitions
        .iter()
        .find(|d| d.characteristics.kind == CardKind::Sorcery)
        .unwrap();
    for mutate in [
        (|d: &mut Definition| d.spell.as_mut().unwrap().timing = SpellTiming::Instant)
            as fn(&mut Definition),
        |d| d.spell.as_mut().unwrap().resolution = SpellResolution::Permanent,
        |d| {
            d.spell.as_mut().unwrap().instructions = vec![Instruction::move_target(
                1,
                kir::Destination::OwnerLibraryTop,
                0,
            )]
        },
        |d| d.spell.as_mut().unwrap().targets.clear(),
    ] {
        let mut bad = ebb.clone();
        mutate(&mut bad);
        assert!(bad.validate().is_err());
    }
}

#[test]
fn creature_lowering_rejects_unrepresented_or_unsupported_complete_behavior() {
    type Mutation = fn(&mut serde_json::Value);
    let mutations: Vec<Mutation> = vec![
        |r| {
            r["source"]["oracle_text"] = "(Flying)".into();
        },
        |r| {
            r["source"]["mana_cost"] = "{X}{G}".into();
        },
        |r| {
            r["source"]["toughness"] = "0".into();
        },
        |r| {
            r["source"]["power"] = "*".into();
        },
        |r| {
            r["source"]["oracle_text"] = "Return target creature to its owner's hand.".into();
            let instant: serde_json::Value = serde_json::from_str(fixtures::RECORDS[3].1).unwrap();
            r["faces"][0]["result"]["lines"] = instant["faces"][0]["result"]["lines"].clone();
            r["faces"][0]["result"]["abilities"] =
                instant["faces"][0]["result"]["abilities"].clone();
        },
    ];
    for mutation in mutations {
        let result = compile(fixtures::MANIFEST, &changed_record(1, mutation));
        assert!(
            !matches!(result, CompileResult::Lowered { .. }),
            "{result:?}"
        );
    }
    let empty_instant = changed_record(3, |r| {
        r["source"]["oracle_text"] = "(Reminder only)".into();
        r["faces"][0]["result"]["lines"] = serde_json::json!([]);
        r["faces"][0]["result"]["abilities"] = serde_json::json!([]);
    });
    assert!(matches!(
        compile(fixtures::MANIFEST, &empty_instant),
        CompileResult::Unsupported { .. }
    ));
}

#[test]
fn ordinary_combat_pool_reuses_complete_vanilla_ir_and_rejects_extra_mechanics() {
    let catalog = embedded_catalog("base").unwrap();
    let bears = catalog
        .definitions
        .iter()
        .find(|definition| definition.oracle_id == "14c8f55d-d177-4c25-a931-ebeb9e6062a0")
        .unwrap();
    assert_eq!(bears.characteristics.power, Some(Quantity::from(2)));
    assert_eq!(bears.characteristics.toughness, Some(Quantity::from(2)));
    assert_eq!(bears.characteristics.mana_cost.generic, Quantity::from(1));
    assert_eq!(bears.characteristics.mana_cost.colored, vec![Color::G]);
    assert!(!bears.characteristics.legendary);
    assert_eq!(bears.provenance.abilities_json, "[]");
    assert_eq!(catalog.schema_version, CURRENT_IR_VERSION);

    // These are rehashed synthetic negative variants of the retained exact
    // source, not additional cards. Account for the complete source block
    // before verifying that unknown keyword/trigger/static semantics reject.
    // Combat rules remain core behavior; they must not make extra source
    // abilities disappear during complete-card lowering.
    for (text, ability) in [
        (
            "Protection from red",
            serde_json::json!({"kind":"keywords","keywords":[{"keyword":"protection"}]}),
        ),
        (
            "Ward {1}",
            serde_json::json!({"kind":"keywords","keywords":[{"keyword":"ward"}]}),
        ),
        (
            "Ward {1}",
            serde_json::json!({"kind":"keywords","keywords":[{"keyword":"ward"}]}),
        ),
        (
            "When this creature dies, draw a card.",
            serde_json::json!({"kind":"triggered","effects":[]}),
        ),
        (
            "Other creatures you control get +1/+1.",
            serde_json::json!({"kind":"static","effects":[]}),
        ),
    ] {
        let changed = changed_record(1, |record| {
            record["source"]["oracle_text"] = text.into();
            record["faces"][0]["result"]["lines"] = serde_json::json!([
                {"text":text,"ok":true,"ability":ability}
            ]);
            record["faces"][0]["result"]["abilities"] = serde_json::json!([ability]);
        });
        assert!(matches!(
            compile(fixtures::MANIFEST, &changed),
            CompileResult::Unsupported { .. }
        ));
    }
    assert!(matches!(
        compile(fixtures::MANIFEST, fixtures::ACTION_RECORDS[11].1),
        CompileResult::Lowered { .. }
    ));
}

#[test]
fn ir_reader_rejects_mismatched_spell_timing_resolution_and_shape() {
    type Mutation = fn(&mut Definition);
    let catalog = embedded_catalog("base").unwrap();
    let creature = catalog
        .definitions
        .iter()
        .find(|d| d.characteristics.kind == CardKind::Creature)
        .unwrap();
    let instant = catalog
        .definitions
        .iter()
        .find(|d| d.characteristics.kind == CardKind::Instant)
        .unwrap();
    let mutations: Vec<Mutation> = vec![
        |d| {
            d.spell = None;
        },
        |d| {
            d.spell.as_mut().unwrap().timing = SpellTiming::Instant;
        },
        |d| {
            d.spell.as_mut().unwrap().resolution = SpellResolution::Instructions;
        },
        |d| {
            d.spell.as_mut().unwrap().functioning_zone = FunctioningZone::Battlefield;
        },
        |d| {
            d.spell
                .as_mut()
                .unwrap()
                .instructions
                .push(Instruction::move_target(0, kir::Destination::OwnerHand, 0));
        },
        |d| {
            d.spell.as_mut().unwrap().targets.push(TargetSpec {
                slot: 0,
                selection_id: "forged".into(),
                chooser: Chooser::Controller,
                stage: EvaluationStage::Announcement,
                filter: None,
                domain: TargetDomain::BattlefieldCreature,
            });
        },
    ];
    for mutation in mutations {
        let mut bad = creature.clone();
        mutation(&mut bad);
        let mut malformed = catalog.clone();
        let target = malformed
            .definitions
            .iter_mut()
            .find(|d| d.oracle_id == bad.oracle_id)
            .unwrap();
        *target = bad;
        assert!(Catalog::read(&canonical(&malformed).unwrap()).is_err());
    }
    for mutation in [
        (|d: &mut Definition| {
            d.spell.as_mut().unwrap().timing = SpellTiming::MainPhase;
        }) as Mutation,
        |d| {
            d.spell.as_mut().unwrap().resolution = SpellResolution::Permanent;
        },
        |d| {
            d.spell.as_mut().unwrap().instructions.clear();
        },
    ] {
        let mut bad = instant.clone();
        mutation(&mut bad);
        assert!(bad.validate().is_err());
    }
    let raw = canonical(&catalog).unwrap();
    for malformed in [
        raw.replace("\"schema_version\":1", "\"schema_version\":2"),
        raw.replace("\"timing\":\"main_phase\"", "\"timing\":\"anytime\""),
        raw.replace(
            "\"resolution\":\"permanent\"",
            "\"resolution\":\"instructions\"",
        ),
        raw.replace(",\"timing\":\"main_phase\"", ""),
        raw.replace("\"resolution\":\"permanent\",", ""),
    ] {
        assert!(malformed != raw, "mutation must change the fixture");
        assert!(Catalog::read(&malformed).is_err());
    }
}

mod combined_hand_cost_fences {
    use super::*;
    #[test]
    fn combined_hand_families_reject_kicker_conditions_without_partial_lowering() {
        for raw in [
            include_str!("../../../../tests/fixtures/36cd2364-d113-47d1-b2c4-b088d9eb88dd.json"),
            include_str!("../../../../tests/fixtures/32e9aa23-fc0a-4b82-82f9-1659c304428c.json"),
            include_str!("../../../../tests/fixtures/7eff84f1-f772-497a-b350-bbc93d0230f7.json"),
        ] {
            for condition in [
                serde_json::json!(null),
                serde_json::json!({"condition":"kicked","what":{"ref":"this","noun":"spell"},"comparison":{"op":"ge","value":{"amount":"fixed","value":1}}}),
            ] {
                let changed = tests::changed_source(raw, |r| {
                    let result = &mut r["faces"][0]["result"];
                    result["abilities"][0]["effects"][0]["condition"] = condition;
                    result["abilities"][0]["effects"][0]["conditionKind"] = serde_json::json!("if");
                    result["lines"][0]["ability"] = result["abilities"][0].clone();
                });
                assert!(!matches!(
                    compile(
                        include_str!("../../../../tests/fixtures/archive-manifest-current.json"),
                        &changed
                    ),
                    CompileResult::Lowered { .. }
                ));
            }
        }
    }
}

#[test]
fn synthetic_cast_color_uses_spell_characteristics() {
    let filter: SourceTypeFilter =
        serde_json::from_str(r#"{"cls":"spell","colors":["blue"]}"#).unwrap();
    let predicate = lower_type_predicate(&filter, Some("spell"), true, 0).unwrap();
    let mut spell: Characteristics = serde_json::from_value(serde_json::json!({
        "kind":"instant", "subtypes":[], "basic":false, "legendary":false,
        "mana_cost":{"generic":"0","colored":[]}, "colors":["R"], "color_identity":["R","U"],
        "power":null, "toughness":null
    }))
    .unwrap();
    assert!(!predicate.matches(&spell));
    spell.colors.push(Color::U);
    assert!(predicate.matches(&spell));
}

#[test]
fn hybrid_announcement_requires_the_exact_source_face_and_empty_ability_graph() {
    let raw = include_str!("../../../../tests/fixtures/synthetic-glaze-lattice.json");
    for mutation in 0..3 {
        let altered = tests::changed_source(raw, |r| match mutation {
            0 => r["binding"]["nodes"][1]["manaCost"] = serde_json::json!("{W/U}{G/W}"),
            1 => r["source"]["mana_cost"] = serde_json::json!("{G/P}{W/U}"),
            _ => r["source"]["oracle_text"] = serde_json::json!("Draw a card."),
        });
        assert!(!matches!(
            compile(fixtures::MANIFEST, &altered),
            CompileResult::Lowered { .. }
        ));
    }
}

#[test]
fn library_top_mana_lowers_only_through_the_typed_r4_pipeline() {
    // The revision-17 row is unparsed: no reparse bypass supplies its AST.
    let header = include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
    let row =
        include_str!("../../../../tests/fixtures/ramos/26445dc7-8363-4205-aee8-1cafeb4ba4c0.json");
    assert!(!matches!(
        compile(header, row),
        CompileResult::Lowered { .. }
    ));
    let r4 = include_str!("../../../../tests/fixtures/archive-manifest-phase1-r4.json");
    let row = include_str!("../../../../tests/fixtures/phase1-r4/library-top-mana.json");
    let CompileResult::Lowered { definition } = compile(r4, row) else {
        panic!("expected the typed R4 composition")
    };
    definition.validate().unwrap();
    assert_eq!(
        definition.characteristics.colored_mana_symbol_count(),
        5.into()
    );
    assert_eq!(
        definition.triggered[0].program.instructions,
        [Instruction::LookTopMayRevealForMana {
            minimum_symbols: 3.into(),
            mana: 3.into(),
            source_block: 1
        }]
    );
    // Any other row, even one changed consistently, is not the accepted row.
    let changed = tests::changed_source(row, |r| {
        r["source"]["oracle_text"] = serde_json::json!(
            r["source"]["oracle_text"]
                .as_str()
                .unwrap()
                .replace("three or more", "four or more")
        )
    });
    assert!(!matches!(
        compile(r4, &changed),
        CompileResult::Lowered { .. }
    ));
    // Every residual is checked, not only the first: a second residual beside
    // the reviewed one, at card or block level, or a second unaccounted step,
    // rejects the source.
    let record: serde_json::Value = serde_json::from_str(row).unwrap();
    let printed = record["faces"][0]["result"]["abilities"].clone();
    let guard = |resolution: &serde_json::Value| {
        let raw = serde_json::to_string(resolution).unwrap();
        let sha = sha256(raw.as_bytes());
        let raw = serde_json::value::RawValue::from_string(raw).unwrap();
        let resolution = resolution::Resolution::read(Some(&raw), Some(&sha))?;
        reviewed_look_top_residual(&resolution, &printed)
    };
    guard(&record["resolution"]).unwrap();
    for second in [
        "/residuals",
        "/faces/0/blocks/1/residuals",
        "/faces/0/blocks/0/residuals",
    ] {
        let mut extra = record["resolution"].clone();
        extra
            .pointer_mut(second)
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!("ref:unreviewed"));
        let error = guard(&extra).unwrap_err();
        assert_eq!(error.reason, "ref:unreviewed", "{second}");
    }
    // The reviewed residual must be the sole unaccounted step at its exact
    // location: the same residual with the step accounted elsewhere rejects.
    let mut step = record["resolution"].clone();
    let only = &mut step["faces"][0]["blocks"][1]["steps"][0];
    *only = serde_json::json!({"path":only["path"].clone(),"astEffect":only["astEffect"].clone(),
        "event":only["astEffect"].clone()});
    assert_eq!(
        guard(&step).unwrap_err().reason,
        "look-top mana source has an unreviewed unaccounted step"
    );
    let mut changed = *definition;
    changed.triggered.clear();
    changed.capabilities = changed.requirements();
    assert!(changed.validate().is_err());
}
