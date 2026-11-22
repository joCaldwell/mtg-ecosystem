//! Residual whole-card families: reviewed mechanics that the block pipeline
//! (`compose`) cannot yet express, and the legacy diagnostic-graph validators
//! (`bindings`, `self_reference`) that pre-R4 frozen fixture rows still need.
//! Each is retired once composition lowers its rows with equivalent IR; the
//! residual waits on the Phase 2C control-node cutover.
use super::*;

pub(crate) mod ability_sequence;
pub(crate) mod activated_artifact_draw;
pub(crate) mod activated_creature_tap;
pub(crate) mod activated_source_keyword;
pub(crate) mod announced_x_draw;
pub(crate) mod announced_x_effects;
pub(crate) mod artifact_loss;
pub(crate) mod artifact_modal;
pub(crate) mod attachment_modifiers;
pub(crate) mod basic_land_fetch;
pub(crate) mod basic_landwalk;
pub(crate) mod bindings;
pub(crate) mod cascade_land_entry;
pub(crate) mod cast_entry;
pub(crate) mod cast_mana_value_sweep;
pub(crate) mod cast_origin;
pub(crate) mod cast_payer_draw;
pub(crate) mod cast_subtype;
pub(crate) mod checkland_entry;
pub(crate) mod colored_library_search;
pub(crate) mod combat_hand_cast;
pub(crate) mod combat_window;
pub(crate) mod commander_modal_keyword;
pub(crate) mod composed_spell;
pub(crate) mod conditional_lands;
pub(crate) mod controlled_creature_spell_copy;
pub(crate) mod controlled_land_entry;
pub(crate) mod controller_counted_draw;
pub(crate) mod controller_creature_pump;
pub(crate) mod controller_land_types;
pub(crate) mod controller_library;
pub(crate) mod controller_window;
pub(crate) mod counter_spell_controller_mill;
pub(crate) mod counter_unless_generic;
pub(crate) mod creature_entry_return;
pub(crate) mod creature_planeswalker_return;
pub(crate) mod cycling;
pub(crate) mod demonstrate;
pub(crate) mod distinct_permanent_colors;
pub(crate) mod divided_plus_one_counters;
pub(crate) mod draw_discard;
pub(crate) mod draw_land_allowance;
pub(crate) mod each_player_draw;
pub(crate) mod entry_copy;
pub(crate) mod entry_graveyard_exile;
pub(crate) mod equipped_trigger;
pub(crate) mod fetched_land_untap;
pub(crate) mod fixed_creature_damage;
pub(crate) mod fixed_mill;
pub(crate) mod fixed_target_modes;
pub(crate) mod granted_land_mana;
pub(crate) mod graveyard_battlefield_return;
pub(crate) mod graveyard_shuffle;
pub(crate) mod greatest_power;
pub(crate) mod group_discard;
pub(crate) mod hand_cast_cascade;
pub(crate) mod hand_return;
pub(crate) mod hybrid_filter_mana;
pub(crate) mod impulse_play;
pub(crate) mod intrinsic_keyword_blocks;
pub(crate) mod keyword_removal_wipe;
pub(crate) mod land_entry_gain_life;
pub(crate) mod land_entry_scry;
pub(crate) mod land_reclamation;
pub(crate) mod land_sacrifice_draw;
pub(crate) mod library_order;
pub(crate) mod library_selection;
pub(crate) mod listed_mana;
pub(crate) mod mana_life_gain;
pub(crate) mod mana_tap_observer;
pub(crate) mod nonland_exile_life;
pub(crate) mod opponent_count_entry;
pub(crate) mod opponent_life_drain;
pub(crate) mod opponent_mana;
pub(crate) mod opponent_nonland_destroy;
pub(crate) mod optional_block;
pub(crate) mod optional_entry_search;
pub(crate) mod optional_trigger;
pub(crate) mod ordinary_sequence;
pub(crate) mod other_land_entry;
pub(crate) mod own_graveyard_card_return;
pub(crate) mod paid_counted_draw;
pub(crate) mod paid_restricted_land;
pub(crate) mod pain_mana;
pub(crate) mod permanent_entry;
pub(crate) mod power_entry_draw;
pub(crate) mod power_filtered_removal;
pub(crate) mod proliferate;
pub(crate) mod repeated_target_modes;
pub(crate) mod resolution_mana;
pub(crate) mod restricted_mana;
pub(crate) mod reveal_land_entry;
pub(crate) mod same_color_mana;
pub(crate) mod scoped_creature_stats;
pub(crate) mod scoped_static;
pub(crate) mod self_graveyard_shuffle;
pub(crate) mod self_reference;
pub(crate) mod self_spell_protection;
pub(crate) mod set_activation_binding;
pub(crate) mod set_exile;
pub(crate) mod source_attack_pump;
pub(crate) mod source_combat_restrictions;
pub(crate) mod source_damage_counters;
pub(crate) mod source_dies_nonland_destroy;
pub(crate) mod source_regeneration;
pub(crate) mod source_tap_damage;
pub(crate) mod spell_cascade_grants;
pub(crate) mod static_permanent_indestructible;
pub(crate) mod successor_tuck;
pub(crate) mod target_color;
pub(crate) mod target_compensation;
pub(crate) mod targeted_counted_draw;
pub(crate) mod targeted_discard;
pub(crate) mod typed_fetchland;
pub(crate) mod typed_land_search;
pub(crate) mod unlimited_hand_size;

pub(crate) fn lower_families(
    archive: &AcceptedArchive,
    record: &str,
) -> Result<Definition, Failure> {
    let r: Record = decode(record, "record")?;
    if r.record_type != "card" {
        return Err(invalid("recordType", "expected card record"));
    }
    if let Some(definition) = greatest_power::lower(&r, archive)? {
        return Ok(definition);
    }
    if r.status != "parsed" {
        return Err(unsupported("status", "complete-card parsing is required"));
    }
    // Archive hashes use JS JSON.stringify. RawValue keeps each exact compact
    // subvalue rather than reordering keys or changing Unicode/number spellings.
    // https://docs.rs/serde_json/latest/serde_json/value/struct.RawValue.html
    if sha256(r.source.get().as_bytes()) != r.source_sha256
        || sha256(r.binding.get().as_bytes()) != r.binding_sha256
    {
        return Err(invalid(
            "record",
            "source or diagnostic binding hash mismatch",
        ));
    }
    // Multi-face Scryfall records intentionally lack single-face oracle_text,
    // mana_cost, etc. Classify the layout before decoding that adapter. Hashes
    // above still bind every source byte; no default fills missing fields.
    let shape: serde_json::Value = decode(r.source.get(), "source")?;
    if shape.get("card_faces").is_some_and(|v| !v.is_null()) {
        if matches!(shape["layout"].as_str(), Some("modal_dfc" | "transform")) {
            return modal_land::lower(&r, &shape, archive);
        }
        let layout = shape["layout"]
            .as_str()
            .ok_or_else(|| invalid("source.layout", "missing layout"))?;
        return Err(unsupported(
            "source.layout",
            format!("multi_face_layout:{layout}"),
        ));
    }
    let source: Source = decode(r.source.get(), "source")?;
    if source.oracle_id != r.oracle_identity || source.id.is_empty() || source.name.is_empty() {
        return Err(invalid("source", "missing or mismatched card identity"));
    }
    if source.loyalty.is_some() {
        return Err(unsupported("source.type_line.card_type", "Planeswalker"));
    }
    if source.defense.is_some() {
        return Err(unsupported("source.type_line.card_type", "Battle"));
    }
    if !(source.layout == "normal"
        || (source.layout == "saga" && source.type_line == "Enchantment — Saga"))
        || source.card_faces.is_some()
        || source.loyalty.is_some()
        || source.defense.is_some()
    {
        return Err(unsupported(
            "source.layout",
            "only complete, single-face normal cards in the initial subset are supported",
        ));
    }
    if r.faces.len() != 1 || r.faces[0].source_face_index != 0 || r.faces[0].status != "attempted" {
        return Err(invalid("faces", "missing or duplicated source face"));
    }
    let result: ParseResult = decode(r.faces[0].result.get(), "faces[0].result")?;
    if !result.ok || result.name != source.name || result.lines.iter().any(|l| !l.ok) {
        return Err(invalid("faces[0].result", "incomplete or mismatched parse"));
    }
    let ast_sha256 = r
        .ast_sha256
        .clone()
        .ok_or_else(|| invalid("astSha256", "missing successful AST hash"))?;
    if sha256(format!("[{}]", result.abilities.get()).as_bytes()) != ast_sha256 {
        return Err(invalid("astSha256", "successful AST hash mismatch"));
    }
    // CR 201.5: canonicalize only exact archive-bound references to this source.
    // Source and AST integrity checks above still cover the original bytes.
    let types = type_line::parse_source(&source.type_line, &source.layout)?;
    let noun = match types.kind {
        CardKind::Creature => "creature",
        CardKind::Artifact => "artifact",
        CardKind::Enchantment if types.subtypes == ["Aura"] => "aura",
        CardKind::Enchantment => "enchantment",
        CardKind::Land => "land",
        CardKind::Instant | CardKind::Sorcery => "spell",
    };
    if result
        .abilities
        .get()
        .contains("\"look-top-may-reveal-for-mana\"")
    {
        if !archive
            .record_sha256
            .contains(&sha256(record.trim_end().as_bytes()))
        {
            return Err(unsupported(
                "record",
                "library-top mana requires its accepted exact archive row",
            ));
        }
        // Archive R4: every reference resolves in the closed vocabulary to the
        // block's controller (CR 109.5), and the resolution covers every
        // reference and step of the AST. The parser leaves only the look-top
        // step's event unaccounted (its sole residual, which `covers` reaches
        // last: the reviewed template has one step in its final block); the
        // template in `archive_definition` proves that step instead.
        let resolution =
            resolution::Resolution::read(r.resolution.as_deref(), r.resolution_sha256.as_deref())?;
        let printed: serde_json::Value = decode(result.abilities.get(), "abilities")?;
        resolution
            .ordinary_antecedents(&printed)
            .map_err(|e| unsupported("binding", e.reason))?;
        reviewed_look_top_residual(&resolution, &printed)?;
        return library_top_mana::archive_definition(SourceProvenance {
            printing_id: source.id,
            source_index: r.source_index,
            source_sha256: r.source_sha256,
            ast_sha256,
            binding_sha256: r.binding_sha256,
            archive_manifest_sha256: archive.manifest_sha256.clone(),
            parser_sha256: archive.parser_source_sha256.clone(),
            source_json: r.source.get().into(),
            abilities_json: result.abilities.get().into(),
        })
        .map_err(|err| unsupported("binding", err));
    }
    let binding = decode(r.binding.get(), "binding")?;
    let intrinsic_keywords =
        intrinsic_keyword_blocks::lower(&source, &shape, types.kind, &result, &binding)?;
    let mut attack_pump =
        source_attack_pump::lower(&source, &shape, types.kind, &result, &binding)?;
    let self_graveyard_shuffle =
        self_graveyard_shuffle::validate(&source, &shape, &result, &binding)?;
    let bestow = bestow::cast_plan(&shape, &result, &binding, &r.oracle_identity)?;
    let canonical_abilities = self_reference::canonicalize(
        result.abilities.get(),
        &binding,
        &r.oracle_identity,
        noun,
        "/faces/0/abilities",
    )?;
    let controller_window =
        controller_window::lower(&shape, &result, &binding, &r.oracle_identity)?;
    let graveyard_shuffle =
        graveyard_shuffle::lower(&shape, &result, &binding, &r.oracle_identity)?;
    let repeated_modes = repeated_target_modes::lower(&shape, &result, &binding, &r)?;
    let fixed_modes = match repeated_modes {
        Some(p) => Some(p),
        None => fixed_target_modes::lower(&shape, &result, &binding, &r.oracle_identity)?,
    };
    let permanent_union =
        creature_planeswalker_return::lower(&shape, &result, &binding, &r.oracle_identity)?;
    let has_linked_hideaway = linked_hideaway::validate(&source, &shape, &result, &binding)?;
    let composition =
        match composed_spell::lower(&shape, &result, &binding, &r.oracle_identity, types.kind)? {
            Some(program) => Some(program),
            None => {
                ordinary_sequence::lower(&shape, &result, &binding, &r.oracle_identity, types.kind)?
            }
        };
    let composition = match composition {
        Some(program) => Some(program),
        None => target_compensation::lower(
            &shape,
            &result,
            &binding,
            &r.oracle_identity,
            types.kind,
            &archive.parser_source_sha256,
        )?,
    };
    let composition = match composition {
        Some(program) => Some(program),
        None => resolution_mana::lower(&shape, &result, &binding, &r.oracle_identity, types.kind)?,
    };
    let composition = match composition {
        Some(program) => Some(program),
        None => {
            combat_window::lower_spell(&shape, &result, &binding, &r.oracle_identity, types.kind)?
        }
    };
    let composition = match composition {
        Some(program) => Some(program),
        None => impulse_play::lower(&shape, &result, &binding, &r.oracle_identity, types.kind)?,
    };
    let composition = match composition {
        Some(program) => Some(program),
        None => optional_block::lower(&shape, &result, &binding, &r.oracle_identity, types.kind)?,
    };
    let mut basic_fetch = basic_land_fetch::activation(&result, &shape, &binding, &source)?;
    let has_conditional_fetch = basic_fetch.as_ref().is_some_and(|activation| {
        matches!(
            activation.program.instructions.as_slice(),
            [Instruction::FetchBasicLand {
                minimum_lands_to_untap: Some(_),
                ..
            }]
        )
    });
    let entrance_noun =
        mtg_contracts::permanent_entry::noun(types.kind, &types.subtypes).or_else(|| {
            (!intrinsic_keywords.is_empty() && types.kind == CardKind::Creature)
                .then_some("creature")
        });
    let abilities = if let Some(ability) =
        proliferate::lower(&source, &result, &shape, &binding, types.kind)?
    {
        vec![ability]
    } else if let Some(activation) = target_color::lower(&source, &shape, &result, &binding)? {
        vec![Ability::BoundActivation(activation)]
    } else if has_conditional_fetch {
        fetched_land_untap::decoded_activation(&canonical_abilities)?
    } else if let Some(program) = composition {
        vec![Ability::ComposedSpell(program)]
    } else if has_linked_hideaway {
        (0..4).map(|_| Ability::LinkedHideaway).collect()
    } else if permanent_union.is_some() {
        vec![Ability::CreaturePlaneswalkerReturn]
    } else if fixed_modes.is_some() {
        vec![Ability::FixedTargetModes]
    } else if graveyard_shuffle.is_some() {
        vec![Ability::PlayerGraveyardShuffle]
    } else if controller_window.is_some() {
        vec![Ability::ControllerWindowBlock; 2]
    } else if self_graveyard_shuffle {
        vec![
            Ability::PrintedProtectionFromEverything,
            Ability::SelfGraveyardRevealShuffle,
        ]
    } else if let Some(abilities) = demonstrate::lower(&source, &shape, &result)? {
        abilities
    } else if let Some(abilities) = hand_cast_cascade::lower(&source, &shape, &result, &binding)? {
        abilities
    } else if let Some(program) = commander_modal_keyword::validate(&shape, &result, &binding)? {
        vec![Ability::CommanderModalKeyword(Box::new(program))]
    } else if divided_plus_one_counters::validate(&shape, &result, &r)? {
        vec![Ability::DividedPlusOneCountersModal]
    } else if let Some(ability) = counter_unless_generic::lower(&shape, &result, &binding)? {
        vec![ability]
    } else if let Some(ability) = cast_payer_draw::lower(&shape, &result, &binding)? {
        vec![ability]
    } else if let Some(ability) = mana_tap_observer::lower(&source, &shape, &result, &binding)? {
        vec![ability]
    } else if controlled_creature_spell_copy::candidate(&source) {
        vec![Ability::ControlledCreatureSpellCopy]
    } else {
        if !intrinsic_keywords.is_empty()
            || result.lines.iter().any(|l| {
                l.text.starts_with(mtg_contracts::equipped_trigger::DAMAGE)
                    || l.text.starts_with(mtg_contracts::equipped_trigger::DIES)
                    || (types.kind == CardKind::Creature
                        && mtg_contracts::permanent_entry::each_creature_sacrifice(&l.text))
                    || mtg_contracts::ability_sequence::candidate(&l.text, types.kind)
                    || mtg_contracts::source_damage_counters::candidate(&l.text)
                    || mtg_contracts::optional_trigger::candidate(&l.text, types.kind)
                    || mtg_contracts::cast_subtype::candidate(&l.text)
                    || (types.kind == CardKind::Land
                        && mtg_contracts::combat_window::split_activation(&l.text).is_some())
                    || entrance_noun.is_some_and(|noun| {
                        mtg_contracts::permanent_entry::candidate(&l.text, noun)
                    })
            })
        {
            let raw: Vec<Box<RawValue>> = decode(&canonical_abilities, "abilities")?;
            if raw.len() != result.lines.len() {
                return Err(invalid("lines", "unaccounted equipped source block"));
            }
            raw.iter()
                .enumerate()
                .map(|(block, raw)| {
                    if let Some(keywords) = intrinsic_keywords.get(&block) {
                        return Ok(Ability::BoundKeywords(keywords.clone()));
                    }
                    let entry_subject = entrance_noun.or_else(|| {
                        (types.kind == CardKind::Creature
                            && mtg_contracts::permanent_entry::each_creature_sacrifice(
                                &result.lines[block].text,
                            ))
                        .then_some("creature")
                    });
                    if let Some(noun) = entry_subject
                        && let Some(a) = permanent_entry::lower(
                            block,
                            &result,
                            &binding,
                            &shape,
                            &r.oracle_identity,
                            noun,
                        )?
                    {
                        return Ok(a);
                    }
                    if let Some(a) = source_damage_counters::lower(
                        block,
                        &result,
                        &binding,
                        &shape,
                        &r.oracle_identity,
                    )? {
                        return Ok(a);
                    }
                    if let Some(a) = optional_entry_search::lower(
                        block,
                        &result,
                        &binding,
                        &shape,
                        &r.oracle_identity,
                        types.kind,
                    )? {
                        return Ok(a);
                    }
                    if let Some(a) = optional_trigger::lower(
                        block,
                        &result,
                        &binding,
                        &shape,
                        &r.oracle_identity,
                        types.kind,
                    )? {
                        return Ok(a);
                    }
                    if let Some(a) =
                        cast_subtype::lower(block, &result, &binding, &shape, &r.oracle_identity)?
                    {
                        return Ok(a);
                    }
                    if types.kind == CardKind::Land
                        && let Some(a) = combat_window::lower_activation(
                            block,
                            &result,
                            &binding,
                            &shape,
                            &r.oracle_identity,
                        )?
                    {
                        return Ok(a);
                    }
                    if let Some(a) = ability_sequence::lower(
                        block,
                        &result,
                        &shape,
                        &binding,
                        &r.oracle_identity,
                        types.kind,
                    )? {
                        return Ok(a);
                    }
                    if let Some(a) = equipped_trigger::lower(
                        block,
                        &result,
                        &binding,
                        &r.oracle_identity,
                        &shape,
                    )? {
                        Ok(Ability::BoundTrigger(a))
                    } else {
                        serde_json::from_str(raw.get()).map_err(|_| {
                            diagnostics::ability(raw.get(), &format!("abilities[{block}]"))
                        })
                    }
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            read_abilities(&canonical_abilities)?
        }
    };
    let land_reclamation = land_reclamation::bound(&source, &shape, &result, &binding)?;
    let restricted_mana = restricted_mana::lower(&source, &shape, &result, &binding)?;
    let mana_life_gain = mana_life_gain::lower(&source, &shape, &result, &binding)?;
    let paid_restricted_land = paid_restricted_land::lower(&source, &shape, &result, &binding)?;
    let has_paid_restricted_land = paid_restricted_land.is_some();
    let mana_haste = restricted_mana
        .as_ref()
        .is_some_and(|ability| ability.source_block == 1);
    restricted_mana::reject_unbound(
        &abilities,
        restricted_mana.is_some() || has_paid_restricted_land,
    )?;
    let mut opponent_entry_mana = opponent_count_entry::bind(
        &abilities,
        result.abilities.get(),
        &shape,
        &binding,
        &r.oracle_identity,
    )?;
    let disjoint_mana =
        match hybrid_filter_mana::lower(&abilities, &result, &shape, &binding, &source)? {
            Some(filter) => Some(filter),
            None => pain_mana::lower(&abilities, &result, &binding, &r.oracle_identity, &source)?,
        };
    let flash_end_step_draw = unlimited_hand_size::is_flash_end_step_draw_candidate(&abilities);
    // CR 207.2c: labels carry no rules meaning. All underlying fields must
    // still decode and lower, including conditions, costs and player relations.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    for ability in &abilities {
        if let Ability::Static {
            effect: effect @ SourceStaticEffect::CombatRule { .. },
            ..
        } = ability
            && source_combat_restrictions::restriction_from_ast(effect).is_none()
        {
            return Err(unsupported("abilities.static", "static:combat-rule"));
        }
        let (Ability::AdditionalCost { ability_word, .. }
        | Ability::Keywords { ability_word, .. }
        | Ability::Static { ability_word, .. }
        | Ability::Spell { ability_word, .. }
        | Ability::Activated { ability_word, .. }
        | Ability::Triggered { ability_word, .. }) = ability
        else {
            continue;
        };
        if ability_word.as_ref().is_some_and(|s| s.trim().is_empty()) {
            return Err(unsupported("abilities.abilityWord", "empty annotation"));
        }
    }
    // Only understood whole-card ASTs reach source normalization. Unsupported
    // nodes are rejected first; every candidate for admission still proves the
    // exact complete normalized source and line/aggregate mirror below.
    if source_lines(&source.oracle_text, &source.name, &source.type_line)?
        != result
            .lines
            .iter()
            .map(|l| normalization::literal_spelling(&l.text))
            .collect::<Vec<_>>()
    {
        return Err(invalid(
            "lines",
            "source contains unaccounted or mismatched normalized blocks",
        ));
    }
    if result.lines.len() != abilities.len()
        && !matches!(abilities.as_slice(), [Ability::ComposedSpell(_)])
    {
        return Err(invalid("lines", "unaccounted ability block"));
    }
    for (block, ability) in abilities.iter().enumerate() {
        if let Ability::Static { effect, .. } = ability {
            if !has_paid_restricted_land {
                conditional_lands::bind(
                    effect,
                    &binding,
                    &r.oracle_identity,
                    source_block(block)?,
                    &source,
                )?;
            }
            spell_cascade_grants::bind(
                effect,
                &binding,
                &r.oracle_identity,
                source_block(block)?,
                &source,
            )?;
            static_permanent_indestructible::bind(
                effect,
                &binding,
                &source,
                block,
                &result.lines[block].text,
            )?;
        }
    }
    if !has_paid_restricted_land {
        conditional_lands::validate_binding_closure(&abilities, &binding, &r.oracle_identity)?;
    }
    let basic_ramp_spell = basic_land_fetch::spell(&result, &shape, &binding, &source)?;
    if basic_fetch.is_none() {
        typed_fetchland::bind(&abilities, &binding, &r.oracle_identity, &source)?;
    }
    cycling::bind(&abilities, &binding, &r.oracle_identity, &source, &result)?;
    let original: Vec<Box<RawValue>> = decode(result.abilities.get(), "abilities")?;
    if original.len() != result.lines.len() {
        return Err(invalid("lines", "unaccounted archived ability block"));
    }
    for (line, ability) in result.lines.iter().zip(&original) {
        let a: serde_json::Value = decode(line.ability.get(), "lines.ability")?;
        let b: serde_json::Value = decode(ability.get(), "abilities")?;
        if a != b || line.text.is_empty() {
            return Err(invalid("lines", "line and aggregate ability disagree"));
        }
    }
    cascade_land_entry::validate(&abilities, &result, &shape, &binding, &source)?;
    entry_copy::validate_binding(&abilities, &source.oracle_id, &shape, &binding)?;
    let mut artifact_loss = artifact_loss::lower(&source, &shape, &result, &binding, &abilities)?;
    let mut counted_artifact_modes =
        artifact_modal::bind(&result, &shape, &binding, &r.oracle_identity)?;
    let opponent_land_mana =
        opponent_mana::lower(&abilities, &result, &binding, &r.oracle_identity, &source)?;
    let (color_stats, color_mana) =
        distinct_permanent_colors::lower(&abilities, &result, &binding, &source)?
            .map_or((None, None), |(stats, mana)| (Some(stats), Some(mana)));
    let granted_mana = granted_land_mana::lower(&source, &shape, &result, &binding)?;
    let grants_land_mana = granted_mana.is_some();
    let mut source_tap_trigger = source_tap_damage::lower(&abilities, &result, &binding, &source)?;
    nonland_exile_life::validate_binding(&source, &result, &binding)?;
    let set_exile = set_exile::lower(&shape, &result, &binding, &r.oracle_identity)?;
    let colored_library = colored_library_search::lower(&source, &shape, &result, &binding)?;
    let opponent_nonland_destroy = opponent_nonland_destroy::lower(&source, &binding)?;
    let keyword_removal_wipe = keyword_removal_wipe::lower(&source, &shape, &result, &binding)?;
    source_dies_nonland_destroy::bind(&source, &result, &binding)?;
    let mut draw_land_allowance =
        if land_reclamation || matches!(abilities.as_slice(), [Ability::ComposedSpell(_)]) {
            None
        } else {
            draw_land_allowance::lower(&abilities, &result, &binding, &source)?
        };
    for (ability, raw) in abilities.iter().zip(&original) {
        typed_land_search::validate_raw_actors(ability, raw.get())?;
        if basic_fetch.is_none() {
            typed_fetchland::validate_raw(ability, raw.get())?;
        }
    }
    controller_library::validate_fixed_cast_effects(
        &abilities,
        &result,
        &binding,
        &r.oracle_identity,
        &shape,
    )?;
    cast_origin::bind(&abilities, &binding, &source, &result)?;
    successor_tuck::bind(&abilities, &binding, &source, &result)?;
    let mut power_removal = power_filtered_removal::lower(&shape, &result, &binding)?;
    let counter_mill = counter_spell_controller_mill::lower(&shape, &result, &binding)?;
    let each_player_draw = each_player_draw::lower(&shape, &result, &binding)?;
    let drain = opponent_life_drain::lower(&shape, &result, &binding)?;
    let announced_x_effects = announced_x_effects::validate(&result, &shape, &binding, types.kind)?;
    let group_discard = group_discard::lower(&result, &shape, &binding, types.kind)?;
    let discard_target = if matches!(types.kind, CardKind::Instant | CardKind::Sorcery)
        && let [Ability::Spell { .. }] = abilities.as_slice()
    {
        targeted_discard::validate(&source, &result, &binding)?
    } else {
        None
    };
    let control_target = abilities.iter().find_map(|a| match a {
        Ability::Static {
            effect: SourceStaticEffect::Control { what, .. },
            ..
        } => Some(what.noun),
        _ => None,
    });
    if let Some(target) = control_target {
        // This family binds the complete source, never an approximated control
        // fragment. CR 303.4a/303.4m, 109.5, 611.3 and 613.1b (2026-09-25).
        // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
        let enchant = matches!(abilities.first(), Some(Ability::Keywords { keywords, .. })
            if matches!(keywords.as_slice(), [k] if k.keyword == "enchant" && k.cost.is_none()
                && AuraControlTarget::from_filter(&k.filter) == Some(target)))
            && result.lines.first().is_some_and(|line| {
                normalization::literal_spelling(&line.text) == format!("Enchant {}", target.noun())
            });
        let controls = abilities
            .iter()
            .filter(|a| {
                matches!(
                    a,
                    Ability::Static {
                        effect: SourceStaticEffect::Control { .. },
                        ..
                    }
                )
            })
            .count();
        // Bind the complete typed family, not a whole-Oracle-text whitelist.
        // Optional modifiers use the same literal attached-creature operand;
        // lower_static and contracts retain their existing modifier validation.
        let effects = abilities
            .iter()
            .zip(&result.lines)
            .skip(1)
            .all(|(ability, line)| {
                match ability {
                    Ability::Static {
                        effect: SourceStaticEffect::Control { who, what },
                        ..
                    } => {
                        // Preserve the reviewed control sentence boundary: a parser
                        // control fragment must not swallow an extra source clause.
                        who.player == "you"
                            && what.reference == "enchanted"
                            && what.noun == target
                            && normalization::literal_spelling(&line.text)
                                == format!("You control enchanted {}.", target.noun())
                    }
                    Ability::Static {
                        effect: SourceStaticEffect::Modify { what, .. },
                        ..
                    } => {
                        target == AuraControlTarget::Creature
                            && *what == serde_json::json!({"ref":"enchanted","noun":"creature"})
                    }
                    _ => false,
                }
            });
        if !enchant || controls != 1 || !(2..=3).contains(&abilities.len()) || !effects {
            return Err(unsupported(
                "static.control",
                "unsupported complete Aura control source",
            ));
        }
    }
    let kind = types.kind;
    let fixed_creature_damage =
        fixed_creature_damage::lower(&shape, &result, &binding, &r.oracle_identity)?;
    // CR118.6/118.6a: no mana cost is distinct from a printed {0}. Until the
    // IR represents absence explicitly, reject nonland sources rather than
    // granting ordinary casting or silently treating a free hit as cost zero.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if kind != CardKind::Land && source.mana_cost.is_empty() {
        return Err(unsupported(
            "source.mana_cost",
            "nonland without a mana cost requires an explicit absent-cost representation",
        ));
    }
    let basic = types.basic;
    let power = source
        .power
        .map(|s| s.parse::<Quantity>())
        .transpose()
        .map_err(|e| unsupported("source.power", e))?;
    let toughness = source
        .toughness
        .map(|s| s.parse::<Quantity>())
        .transpose()
        .map_err(|e| unsupported("source.toughness", e))?;
    if source.mana_cost.matches("{X}").count() > 1
        || (source.mana_cost.contains("{X}")
            && !matches!(kind, CardKind::Instant | CardKind::Sorcery))
    {
        return Err(unsupported("mana_cost", "unsupported X cost"));
    }
    let saga = saga::bind(&shape, &r.oracle_identity, &binding, &result, &abilities)?;
    let mut labels = SourceLabels {
        text: source.oracle_text.clone(),
        blocks: result.lines.iter().map(|l| l.text.clone()).collect(),
        ..SourceLabels::default()
    };
    let mana_abilities: Vec<_> = disjoint_mana
        .into_iter()
        .flatten()
        .chain(opponent_land_mana)
        .chain(color_mana)
        .chain(granted_mana)
        .chain(restricted_mana)
        .chain(paid_restricted_land.into_iter().flatten())
        .chain(mana_life_gain)
        .collect();
    labels
        .mana
        .extend(mana_abilities.iter().map(|a| a.source_block));
    let opening_composition = three_static_ast(&source.name, &source.oracle_text);
    let (base_cost, hybrid_mana) =
        if let Some((cost, symbols)) = color_hybrid_cost(&source.mana_cost) {
            if let Some(expected) = &opening_composition {
                if *expected != decode::<serde_json::Value>(result.abilities.get(), "abilities")? {
                    return Err(unsupported(
                        "abilities",
                        "controller composition does not match source",
                    ));
                }
            } else if source.oracle_text.is_empty() && abilities.is_empty() {
                bindings::Bindings::new(&r.oracle_identity, &shape).validate(
                    &binding,
                    "hybrid cost requires the exact printed face binding",
                )?;
            } else {
                let path = "source.mana_cost";
                if kind != CardKind::Creature {
                    return Err(unsupported(
                        path,
                        "hybrid ability composition is unsupported",
                    ));
                }
                let keyword_blocks = |text: &str| {
                    hybrid_intrinsic_keywords(&source.mana_cost, text, result.abilities.get())
                        .map_err(|message| unsupported(path, message))
                };
                let printed = keyword_blocks(&source.oracle_text)?;
                if keyword_blocks(&labels.blocks.join("\n"))? != printed {
                    return Err(unsupported(
                        path,
                        "hybrid keyword labels differ from the source",
                    ));
                }
                basic_landwalk::bind_intrinsic_keywords(
                    &r.oracle_identity,
                    &shape,
                    &binding,
                    &printed,
                    path,
                    "requires complete intrinsic hybrid creature bindings",
                )?;
            }
            (cost, symbols)
        } else {
            (mana_cost(&source.mana_cost.replace("{X}", ""))?, vec![])
        };
    let mut definition = Definition {
        modal_land: None,
        transform_faces: None,
        source_labels: None,
        x_mana_cost: source.mana_cost.contains("{X}"),
        oracle_id: r.oracle_identity,
        name: source.name,
        characteristics: Characteristics {
            kind,
            additional_types: types.additional_types,
            basic,
            legendary: types.legendary,
            snow: types.snow,
            subtypes: types.subtypes,
            hybrid_mana,
            mana_cost: base_cost,
            colors: source.colors,
            color_identity: source.color_identity,
            power,
            toughness,
        },
        composed: false,
        intrinsic_zone: if kind == CardKind::Land {
            Some(FunctioningZone::Battlefield)
        } else {
            None
        },
        spell: None,
        activated: vec![],
        mana_abilities,
        triggered: vec![],
        static_abilities: vec![],
        keywords: vec![],
        capabilities: BTreeSet::new(),
        provenance: SourceProvenance {
            printing_id: source.id,
            source_index: r.source_index,
            source_sha256: r.source_sha256,
            ast_sha256,
            binding_sha256: r.binding_sha256,
            archive_manifest_sha256: archive.manifest_sha256.clone(),
            parser_sha256: archive.parser_source_sha256.clone(),
            source_json: r.source.get().into(),
            abilities_json: result.abilities.get().into(),
        },
    };
    if mana_haste {
        definition.keywords.push(KeywordAbility::Haste);
    }
    match kind {
        CardKind::Sorcery | CardKind::Instant if basic_ramp_spell.is_some() => {
            definition.spell = basic_ramp_spell;
        }
        CardKind::Sorcery | CardKind::Instant if fixed_creature_damage.is_some() => {
            definition.spell = fixed_creature_damage;
        }
        CardKind::Sorcery | CardKind::Instant if permanent_union.is_some() => {
            definition.spell = permanent_union;
        }
        CardKind::Sorcery | CardKind::Instant if fixed_modes.is_some() => {
            definition.spell = fixed_modes;
        }
        CardKind::Sorcery | CardKind::Instant if graveyard_shuffle.is_some() => {
            definition.spell = graveyard_shuffle;
        }
        CardKind::Sorcery | CardKind::Instant if controller_window.is_some() => {
            definition.spell = controller_window;
        }
        CardKind::Sorcery | CardKind::Instant if set_exile.is_some() => {
            definition.spell = set_exile;
        }
        CardKind::Sorcery if keyword_removal_wipe.is_some() => {
            definition.spell = keyword_removal_wipe;
        }
        CardKind::Sorcery if colored_library.is_some() => {
            definition.spell = colored_library;
        }
        CardKind::Sorcery if opponent_nonland_destroy.is_some() => {
            definition.spell = opponent_nonland_destroy;
        }
        CardKind::Land | CardKind::Creature | CardKind::Artifact | CardKind::Enchantment => {
            if kind == CardKind::Land && basic && !abilities.is_empty() {
                return Err(unsupported(
                    "abilities",
                    "nonempty land abilities are not supported",
                ));
            }
            if kind == CardKind::Creature
                && abilities.is_empty()
                && definition.keywords.is_empty()
                && !source.oracle_text.is_empty()
            {
                return Err(invalid(
                    "source.oracle_text",
                    "empty AST for nonempty creature behavior",
                ));
            }
            if kind != CardKind::Land {
                // CR 302.1–2 / 608.3a: casting a creature and activating one of
                // its battlefield abilities are distinct executable programs.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                definition.spell = Some(SpellProgram {
                    optional_additional_costs: vec![],
                    additional_sacrifice: None,
                    timing: SpellTiming::MainPhase,
                    resolution: SpellResolution::Permanent,
                    functioning_zone: FunctioningZone::Stack,
                    targets: vec![],
                    instructions: vec![],
                    modes: vec![],
                });
            }
            let remaining_abilities = if land_reclamation {
                labels.statics.extend([0, 1]);
                labels.triggered.push(2);
                definition
                    .static_abilities
                    .extend(land_reclamation_statics());
                definition.triggered.push(land_reclamation_trigger());
                &[]
            } else if let Some(chapters) = &saga {
                for chapter in chapters {
                    for ability in chapter.abilities() {
                        labels.triggered.push(chapter.source_block);
                        definition.triggered.push(ability);
                    }
                }
                &abilities[abilities.len()..]
            } else if has_paid_restricted_land {
                labels.statics.push(0);
                definition
                    .static_abilities
                    .push(mtg_contracts::paid_restricted_land::entry());
                &[]
            } else if has_linked_hideaway {
                use mtg_contracts::linked_hideaway;
                labels.triggered.push(0);
                labels.statics.push(1);
                labels.mana.push(2);
                labels.activated.push(3);
                definition.triggered.push(linked_hideaway::entry());
                definition.static_abilities.push(linked_hideaway::tapped());
                definition.mana_abilities.push(linked_hideaway::mana());
                definition.activated.push(linked_hideaway::play());
                &[]
            } else if grants_land_mana {
                labels.statics.push(0);
                definition
                    .static_abilities
                    .push(mtg_contracts::granted_land_mana_static());
                &[]
            } else if let Some(stats) = color_stats {
                // The complete family consumed the static and activated blocks.
                labels.statics.push(1);
                definition.static_abilities.push(stats);
                &abilities[..1]
            } else if definition.mana_abilities.is_empty() {
                abilities.as_slice()
            } else {
                // The other whole-source mana families consumed every block.
                &[]
            };
            for (block, ability) in remaining_abilities.iter().enumerate() {
                match ability {
                    Ability::BoundKeywords(keywords) => definition.keywords.extend(keywords),
                    Ability::PrintedProtectionFromEverything => {
                        definition
                            .keywords
                            .push(KeywordAbility::ProtectionFromEverything);
                    }
                    Ability::SelfGraveyardRevealShuffle => {
                        labels.statics.push(source_block(block)?);
                        definition
                            .static_abilities
                            .push(mtg_contracts::self_graveyard_shuffle::static_ability());
                    }
                    Ability::HandCastCascade => {
                        labels.triggered.push(source_block(block)?);
                        definition
                            .triggered
                            .push(mtg_contracts::hand_cast_cascade::hand_cast_cascade_ability());
                    }
                    Ability::CastPayerDraw { generic } => {
                        labels.triggered.push(source_block(block)?);
                        definition
                            .triggered
                            .push(cast_payer_draw_trigger(generic.clone()));
                    }
                    Ability::Static { effect, .. } => {
                        labels.statics.push(source_block(block)?);
                        let ability = match source_combat_restrictions::lower(
                            effect, block, &result, &shape, &binding, kind,
                        )? {
                            Some(ability) => ability,
                            None => match scoped_creature_stats::lower(
                                effect,
                                block,
                                &result,
                                &shape,
                                &binding,
                                &definition.oracle_id,
                                kind,
                            )? {
                                Some(ability) => ability,
                                None => match scoped_static::lower(
                                    block,
                                    &result,
                                    &shape,
                                    &binding,
                                    &definition.oracle_id,
                                )? {
                                    Some(ability) => ability,
                                    None => match attachment_modifiers::lower(
                                        block,
                                        &result,
                                        &shape,
                                        &binding,
                                        &definition.oracle_id,
                                    )? {
                                        Some(ability) => ability,
                                        None => lower_static(effect, source_block(block)?)?,
                                    },
                                },
                            },
                        };
                        definition.static_abilities.push(ability);
                    }
                    Ability::Keywords { keywords, .. } => {
                        if keywords.is_empty() {
                            return Err(unsupported("abilities.keywords", "empty keyword block"));
                        }
                        for (keyword_index, keyword) in keywords.iter().enumerate() {
                            if keyword.keyword == "bestow"
                                && let Some(plan) = &bestow
                            {
                                labels.statics.push(source_block(block)?);
                                definition.static_abilities.push(StaticAbility {
                                    functioning_zone: FunctioningZone::Stack,
                                    effect: StaticEffect::Bestow {
                                        cost: plan.base_cost(BestowForm::Aura).clone(),
                                        program: canonical_bestow_program(),
                                    },
                                });
                                continue;
                            }
                            if keyword.keyword == "cycling" {
                                labels.activated.push(source_block(block)?);
                                definition
                                    .activated
                                    .push(cycling::lower(keyword, source_block(block)?)?);
                                continue;
                            }
                            if keyword.keyword == "equip"
                                && kind == CardKind::Artifact
                                && definition.characteristics.subtypes == ["Equipment"]
                                && keyword.filter.is_none()
                            {
                                let costs = keyword
                                    .cost
                                    .as_ref()
                                    .ok_or_else(|| unsupported("equip", "missing equip cost"))?;
                                let c = lower_permanent_costs(costs, kind)?;
                                if c.tap || c.sacrifice || c.mana.is_none() {
                                    return Err(unsupported(
                                        "equip",
                                        "equip requires fixed mana only",
                                    ));
                                }
                                labels.activated.push(source_block(block)?);
                                definition.activated.push(
                                    mtg_contracts::attachment_modifiers::equip_ability(
                                        c.mana.unwrap(),
                                        source_block(block)?,
                                    ),
                                );
                                continue;
                            }
                            if keyword.keyword == "enchant"
                                && kind == CardKind::Enchantment
                                && definition.characteristics.subtypes == ["Aura"]
                                && keyword.cost.is_none()
                                && AuraControlTarget::from_filter(&keyword.filter).is_some_and(
                                    |t| {
                                        t == AuraControlTarget::Creature
                                            || control_target == Some(t)
                                    },
                                )
                            {
                                let target =
                                    AuraControlTarget::from_filter(&keyword.filter).unwrap();
                                let spell = definition.spell.as_mut().unwrap();
                                if !spell.targets.is_empty() {
                                    return Err(unsupported("enchant", "duplicate enchant"));
                                }
                                let selection = format!("enchanted-{}", target.noun());
                                spell
                                    .targets
                                    .push(if target == AuraControlTarget::Creature {
                                        TargetSpec::new(
                                            0,
                                            selection,
                                            TargetDomain::BattlefieldCreature,
                                        )
                                    } else {
                                        TargetSpec::new(
                                            0,
                                            selection,
                                            TargetDomain::BattlefieldPermanent,
                                        )
                                        .with_filter(
                                            PermanentFilter::any().with_types(target.kind()),
                                        )
                                    });
                                continue;
                            }
                            if keyword.keyword == "affinity" {
                                // CR 702.41a/b: complete exact filter and prospective caster.
                                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                                if keyword.cost.is_some()
                                    || keyword.filter
                                        != Some(serde_json::json!({"types":["artifact"]}))
                                {
                                    return Err(unsupported(
                                        "affinity",
                                        "unsupported exact affinity selector",
                                    ));
                                }
                                let binding: serde_json::Value =
                                    decode(r.binding.get(), "binding")?;
                                let path =
                                    format!("/faces/0/abilities/{block}/keywords/{keyword_index}");
                                let scope = format!(
                                    "card:{}/faces/0/abilities/{block}#ability",
                                    definition.oracle_id
                                );
                                let you = format!("card:{}{path}#you", definition.oracle_id);
                                let id = format!("card:{}{path}#keyword", definition.oracle_id);
                                let nodes = binding["nodes"].as_array().ok_or_else(|| {
                                    unsupported("binding.affinity", "missing nodes")
                                })?;
                                let source_id =
                                    format!("card:{}/source#object", definition.oracle_id);
                                let face_id = format!("card:{}/faces/0#face", definition.oracle_id);
                                let required = [
                                    serde_json::json!({"id":source_id,"path":"/source","node":"source","identity":definition.oracle_id}),
                                    serde_json::json!({"id":face_id,"path":"/faces/0","node":"face","source":source_id,"sourceFaceIndex":0,"name":definition.name,"layout":source.layout,"typeLine":source.type_line,"manaCost":source.mana_cost}),
                                    serde_json::json!({"id":scope,"path":format!("/faces/0/abilities/{block}"),"node":"ability","source":source_id,"face":face_id,"abilityKind":"keywords"}),
                                ];
                                if binding["identity"] != definition.oracle_id
                                    || required.iter().any(|expected| {
                                        let found: Vec<_> = nodes
                                            .iter()
                                            .filter(|n| n["id"] == expected["id"])
                                            .collect();
                                        found.len() != 1
                                            || expected
                                                .as_object()
                                                .unwrap()
                                                .iter()
                                                .any(|(k, v)| found[0][k] != *v)
                                    })
                                    || nodes.iter().filter(|n| n["id"] == id).count() != 1
                                    || nodes.iter().filter(|n| n["id"] == you).count() != 1
                                    || !nodes.iter().any(|n| {
                                        n["id"] == id
                                            && n["path"] == path
                                            && n["scope"] == scope
                                            && n["node"] == "keyword-ability"
                                            && n["keyword"] == "affinity"
                                            && n["abilities"] == serde_json::json!(["static"])
                                            && n["you"] == you
                                    })
                                    || !nodes.iter().any(|n| {
                                        n["id"] == you
                                            && n["path"] == path
                                            && n["scope"] == scope
                                            && n["node"] == "runtime-player"
                                            && n["role"] == "prospective-caster"
                                    })
                                {
                                    return Err(unsupported(
                                        "binding.affinity",
                                        "missing exact prospective caster binding",
                                    ));
                                }
                                labels.statics.push(source_block(block)?);
                                definition.static_abilities.push(StaticAbility {
                                    functioning_zone: FunctioningZone::Stack,
                                    effect: StaticEffect::ReduceSelfGenericCost {
                                        per_object: 1.into(),
                                        count: BattlefieldCount {
                                            kind: CardKind::Artifact,
                                            controller: CountController::Caster,
                                        },
                                    },
                                });
                                continue;
                            }
                            if keyword.cost.is_some() || keyword.filter.is_some() {
                                return Err(unsupported(
                                    "keyword",
                                    format!("keyword:{}:parameters", keyword.keyword),
                                ));
                            }
                            match keyword.keyword.as_str() {
                                "cascade" if kind != CardKind::Land => {
                                    // CR 702.85c: each exact keyword occurrence is
                                    // independent, even inside one source block.
                                    labels.triggered.push(source_block(block)?);
                                    definition.triggered.push(TriggeredAbility {
                                        functioning_zone: FunctioningZone::Stack,
                                        condition: TriggerCondition::SelfSpellCast,
                                        intervening_if: None,
                                        death_filter: None,
                                        program: canonical_cascade_program(source_block(block)?),
                                    });
                                }
                                "flash" => definition.keywords.push(KeywordAbility::Flash),
                                "defender" => definition.keywords.push(KeywordAbility::Defender),
                                "trample" => definition.keywords.push(KeywordAbility::Trample),
                                "first strike" => {
                                    definition.keywords.push(KeywordAbility::FirstStrike)
                                }
                                "double strike" => {
                                    definition.keywords.push(KeywordAbility::DoubleStrike)
                                }
                                "menace" => definition.keywords.push(KeywordAbility::Menace),
                                "hexproof" => definition.keywords.push(KeywordAbility::Hexproof),
                                "shroud" => definition.keywords.push(KeywordAbility::Shroud),
                                "flying" => definition.keywords.push(KeywordAbility::Flying),
                                "reach" => definition.keywords.push(KeywordAbility::Reach),
                                "vigilance" => definition.keywords.push(KeywordAbility::Vigilance),
                                "haste" => definition.keywords.push(KeywordAbility::Haste),
                                "lifelink" => definition.keywords.push(KeywordAbility::Lifelink),
                                "deathtouch" => {
                                    definition.keywords.push(KeywordAbility::Deathtouch)
                                }
                                "indestructible" => {
                                    definition.keywords.push(KeywordAbility::Indestructible)
                                }
                                _ => {
                                    return Err(unsupported(
                                        "abilities.keywords",
                                        format!("keyword:{}", keyword.keyword),
                                    ));
                                }
                            }
                        }
                    }
                    Ability::Activated {
                        costs,
                        effects,
                        restriction,
                        ..
                    } => {
                        let mana = match opponent_entry_mana.take() {
                            Some(mana) => Some(mana),
                            None => same_color_mana::lower(
                                ability,
                                &result,
                                &shape,
                                &binding,
                                &definition.oracle_id,
                                &source.oracle_text,
                            )?,
                        };
                        let mana = match mana {
                            Some(mana) => Some(mana),
                            None => listed_mana::lower(
                                ability,
                                block,
                                &result.lines[block].text,
                                &shape,
                                &binding,
                                &definition.oracle_id,
                            )?,
                        };
                        let mana = match mana {
                            Some(mana) => Some(mana),
                            None => lower_mana_ability(
                                costs,
                                effects,
                                restriction.as_ref(),
                                block,
                                kind,
                            )?,
                        };
                        if let Some(mana) = mana {
                            labels.mana.push(source_block(block)?);
                            definition.mana_abilities.push(mana);
                        } else {
                            if restriction.is_some() {
                                return Err(unsupported(
                                    "restriction",
                                    "restriction requires counter-removal mana",
                                ));
                            }
                            labels.activated.push(source_block(block)?);
                            let fixed = fixed_creature_activation(
                                &result.lines[block].text,
                                "~",
                                source_block(block)?,
                            );
                            let activated = if let Some(fetch) = basic_fetch.take() {
                                fetch
                            } else if let Some(loss) = artifact_loss.take() {
                                loss
                            } else if let Some(bound) = counted_artifact_modes.remove(&block) {
                                bound
                            } else if let Some(ability) = activated_source_keyword::lower(
                                &result.lines[block],
                                block,
                                &original[block],
                                &shape,
                                &binding,
                                kind,
                            )? {
                                ability
                            } else if let Some(spec) = fixed {
                                let raw: serde_json::Value =
                                    decode(original[block].get(), "activated")?;
                                if raw != spec.ast {
                                    return Err(unsupported(
                                        "activation",
                                        "printed fixed activation differs from AST",
                                    ));
                                }
                                spec.ability
                            } else {
                                lower_activation(costs, effects, block, kind)?
                            };
                            definition.activated.push(activated);
                        }
                    }
                    Ability::BoundActivation(ability) => {
                        labels.activated.push(source_block(block)?);
                        definition.activated.push(ability.clone());
                    }
                    Ability::BoundTrigger(ability) => {
                        labels.triggered.push(source_block(block)?);
                        definition.triggered.push(ability.clone());
                    }
                    Ability::Triggered {
                        trigger,
                        effects,
                        condition,
                        ability_word: _,
                    } => {
                        labels.triggered.push(source_block(block)?);
                        let ability = if let Some(pump) = attack_pump.take() {
                            pump
                        } else if block == 1
                            && condition.is_none()
                            && combat_hand_cast::is_offer(effects)
                        {
                            mtg_contracts::combat_hand_cast_trigger()
                        } else if let Some(ability) = source_tap_trigger.take() {
                            ability
                        } else if let Some(discard) = &group_discard {
                            group_discard::entry_trigger(discard.clone())
                        } else if flash_end_step_draw {
                            unlimited_hand_size::lower_end_step_draw(
                                trigger,
                                effects,
                                condition.as_ref(),
                                block,
                            )?
                        } else {
                            lower_trigger(trigger, effects, condition.as_ref(), block)?
                        };
                        definition.triggered.push(ability);
                    }
                    _ => {
                        return Err(unsupported(
                            "abilities.kind",
                            "unsupported complete creature ability kind",
                        ));
                    }
                }
            }
        }
        CardKind::Instant | CardKind::Sorcery => {
            if source.oracle_text.is_empty() {
                return Err(invalid("source.oracle_text", "spell has no source text"));
            }
            let mut program = SpellProgram {
                optional_additional_costs: vec![],
                additional_sacrifice: None,
                // CR 304.1 / 307.1: ordinary spell timing follows card type.
                // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
                timing: if kind == CardKind::Instant {
                    SpellTiming::Instant
                } else {
                    SpellTiming::MainPhase
                },
                resolution: SpellResolution::Instructions,
                functioning_zone: FunctioningZone::Stack,
                targets: vec![],
                instructions: vec![],
                modes: vec![],
            };
            optional_costs::declare(
                &abilities,
                &result,
                &binding,
                &definition.oracle_id,
                &mut program,
            )?;
            let mut slots = BTreeMap::new();
            for (block, ability) in abilities.iter().enumerate() {
                if let Ability::ComposedSpell(composition) = ability {
                    program = composition.clone();
                    continue;
                }
                if let Ability::Demonstrate = ability {
                    let source_block = source_block(block)?;
                    definition.triggered.push(TriggeredAbility {
                        functioning_zone: FunctioningZone::Stack,
                        condition: TriggerCondition::SelfSpellCast,
                        intervening_if: None,
                        death_filter: None,
                        program: SpellProgram {
                            timing: SpellTiming::Instant,
                            instructions: vec![Instruction::Demonstrate { source_block }],
                            ..program.clone()
                        },
                    });
                    labels.triggered.push(source_block);
                    continue;
                }
                if let Ability::RevealAndCast = ability {
                    program.instructions.push(Instruction::RevealAndCast {
                        source_block: source_block(block)?,
                    });
                    continue;
                }
                if let Ability::Keywords { keywords, .. } = ability {
                    if matches!(keywords.as_slice(), [k] if k.keyword == "kicker") {
                        continue; // Complete declaration and linked condition checked together.
                    }
                    if let [keyword] = keywords.as_slice()
                        && keyword.keyword == "cycling"
                    {
                        labels.activated.push(source_block(block)?);
                        definition
                            .activated
                            .push(cycling::lower(keyword, source_block(block)?)?);
                        continue;
                    }
                    let previous = definition.triggered.len();
                    optional_costs::instant_keywords(&mut definition, keywords, block)?;
                    labels.triggered.extend(std::iter::repeat_n(
                        source_block(block)?,
                        definition.triggered.len() - previous,
                    ));
                    continue;
                }
                if let Ability::AdditionalCost { costs, .. } = ability {
                    if block != 0 || program.additional_sacrifice.is_some() {
                        return Err(unsupported(
                            "cost",
                            "duplicate or misplaced additional cost",
                        ));
                    }
                    let [SourceCost::Sacrifice { what }] = costs.as_slice() else {
                        return Err(unsupported("cost", "unsupported additional cost"));
                    };
                    program.additional_sacrifice = Some(lower_sacrifice_filter(what)?);
                    continue;
                }
                if let Ability::Static { effect, .. } = ability {
                    let a = lower_static(effect, source_block(block)?)?;
                    if a.functioning_zone != FunctioningZone::Stack {
                        return Err(unsupported("static", "spell static must function on stack"));
                    }
                    labels.statics.push(source_block(block)?);
                    definition.static_abilities.push(a);
                    continue;
                }
                if let Ability::CommanderModalKeyword(lowered) = ability {
                    program = (**lowered).clone();
                    continue;
                }
                if let Ability::DividedPlusOneCountersModal = ability {
                    program = divided_plus_one_counters_modal_program();
                    continue;
                }
                if let Ability::CounterUnlessGeneric { generic } = ability {
                    let target_slot = bind_target(
                        &mut program,
                        &mut slots,
                        counter_unless_generic::SELECTION,
                        TargetDomain::StackSpell,
                    )?;
                    program
                        .instructions
                        .push(Instruction::CounterSpellUnlessPays {
                            target_slot,
                            generic: generic.clone(),
                            source_block: 0,
                        });
                    continue;
                }
                if let Ability::ControlledCreatureSpellCopy = ability {
                    let target_slot = bind_target(
                        &mut program,
                        &mut slots,
                        controlled_creature_spell_copy::SELECTION,
                        TargetDomain::ControlledCreatureSpell,
                    )?;
                    program
                        .instructions
                        .push(Instruction::CopyControlledCreatureSpell {
                            target_slot,
                            source_block: 0,
                        });
                    continue;
                }
                let Ability::Spell { effects, .. } = ability else {
                    return Err(unsupported(
                        "abilities.kind",
                        "nonspell ability on instant/sorcery",
                    ));
                };
                if let Some((target, instruction)) = power_removal.take() {
                    program.targets.push(target);
                    program.instructions.push(instruction);
                    continue;
                }
                if let Some(count) = &counter_mill {
                    let target_slot = bind_target(
                        &mut program,
                        &mut slots,
                        counter_spell_controller_mill::TARGET_ID,
                        TargetDomain::StackSpell,
                    )?;
                    program
                        .instructions
                        .push(Instruction::CounterSpellAndMillController {
                            target_slot,
                            count: count.clone(),
                            source_block: 0,
                        });
                    continue;
                }
                if let Some(count) = &each_player_draw {
                    program.instructions.push(Instruction::act(
                        Verb::Draw {
                            player: PlayerExpr::EachPlayer,
                            amount: count.clone(),
                        },
                        0,
                    ));
                    continue;
                }
                if let Some(instructions) = draw_land_allowance.take() {
                    if program.additional_sacrifice.is_some()
                        || !program.optional_additional_costs.is_empty()
                    {
                        return Err(unsupported(
                            "land allowance",
                            "requires ordinary spell costs",
                        ));
                    }
                    program.instructions.extend(instructions);
                    continue;
                }
                if let Some(discard) = &group_discard {
                    program.instructions.push(discard.clone());
                    continue;
                }
                if let Some(amount) = &drain {
                    program.instructions.push(Instruction::drain(
                        Context::Spell,
                        amount.clone(),
                        0,
                    ));
                    continue;
                }
                if announced_x_effects {
                    append_spell_effects(
                        &mut program,
                        effects,
                        block,
                        &mut slots,
                        PlayerBinding::AnnouncedX,
                        Context::Spell,
                    )?;
                    continue;
                }
                if effects.iter().flat_map(|s| &s.steps).any(|step| {
                    matches!(
                        step,
                        Step::Draw {
                            amount: DrawAmount::X {},
                            ..
                        }
                    )
                }) {
                    if !definition.x_mana_cost || abilities.len() != 1 {
                        return Err(unsupported(
                            "effects.draw",
                            "requires standalone announced X spell",
                        ));
                    }
                    announced_x_draw::lower(
                        result.abilities.get(),
                        &result.lines[0].text,
                        &binding,
                        &definition.oracle_id,
                        &source.mana_cost,
                        &mut program,
                        &mut slots,
                    )?;
                    continue;
                }
                if abilities.len() == 1
                    && targeted_counted_draw::lower(effects, &mut program, &mut slots)?
                {
                    continue;
                }
                if let Some((count, life_loss)) = controller_counted_draw::coalesce(effects)? {
                    if abilities.len() != 1
                        || program.additional_sacrifice.is_some()
                        || !program.optional_additional_costs.is_empty()
                        || block != 0
                        || !program.instructions.is_empty()
                        || !program.targets.is_empty()
                        || !program.modes.is_empty()
                    {
                        return Err(unsupported(
                            "counted draw",
                            "requires complete standalone source",
                        ));
                    }
                    program.instructions.push(Instruction::draw(
                        PlayerExpr::you(Context::Spell),
                        count,
                        0,
                    ));
                    if let Some(amount) = life_loss {
                        program.instructions.push(Instruction::act(
                            Verb::LoseLife {
                                player: PlayerExpr::you(Context::Spell),
                                amount: Amount::Fixed { value: amount },
                            },
                            0,
                        ));
                    }
                    continue;
                }
                if discard_target.is_none()
                    && let Some((draw, count)) = draw_discard::coalesce(effects)?
                {
                    if abilities.len() != 1
                        || program.additional_sacrifice.is_some()
                        || !program.optional_additional_costs.is_empty()
                        || block != 0
                        || !program.instructions.is_empty()
                        || !program.targets.is_empty()
                        || !program.modes.is_empty()
                    {
                        return Err(unsupported(
                            "draw discard",
                            "requires whole standalone source",
                        ));
                    }
                    program.instructions.extend([
                        Instruction::draw(PlayerExpr::you(Context::Spell), draw, 0),
                        Instruction::discard(
                            kir::PlayerExpr::you(kir::Context::Spell),
                            kir::Amount::Fixed { value: count },
                            false,
                            0,
                        ),
                    ]);
                    continue;
                }
                validate_damage_sources(effects, DamageSourceBinding::Spell)?;
                // Separate source blocks introduce independent occurrences (CR115.3).
                // Reusing a selection ID across blocks must not alias targets.
                slots.clear();
                if let Some(domain) = discard_target {
                    if program.additional_sacrifice.is_some()
                        || !program.optional_additional_costs.is_empty()
                    {
                        return Err(unsupported("discard", "requires ordinary spell costs"));
                    }
                    append_spell_effects(
                        &mut program,
                        effects,
                        block,
                        &mut slots,
                        PlayerBinding::DiscardTarget(domain),
                        Context::Spell,
                    )?;
                } else {
                    optional_costs::append(&mut program, effects, block, &mut slots)?;
                }
            }
            // Multiple intrinsic instances on a spell are reviewed only with this
            // complete sweep. Definition validation also binds source, AST and block indices.
            if definition.triggered.len() == 2
                && !matches!(
                    program.instructions.as_slice(),
                    [damage] if damage.set_damage().is_some_and(|d| matches!(
                        d.amount,
                        kir::Amount::OtherCastsManaValue { .. }
                    ))
                )
            {
                return Err(unsupported(
                    "abilities.keywords",
                    "unsupported spell keyword combination",
                ));
            }
            definition.spell = Some(program);
        }
    }
    if definition.keywords.contains(&KeywordAbility::Flash) {
        definition
            .spell
            .as_mut()
            .ok_or_else(|| unsupported("keyword", "flash requires castable spell"))?
            .timing = SpellTiming::Instant;
    }
    if definition.triggered.iter().any(|a| {
        a.condition != TriggerCondition::OpponentCreatureDies
            && a.program
                .instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoseLifeDeathController { .. }))
    }) {
        return Err(unsupported("trigger", "unbound death controller"));
    }
    if definition
        .activated
        .iter()
        .any(|a| sacrifice_source_creature_set_keyword_ability(a).is_some())
    {
        set_activation_binding::bind(
            &binding,
            &definition.oracle_id,
            &definition.name,
            &source.layout,
            &source.type_line,
            &source.mana_cost,
            &canonical_abilities,
        )?;
    }
    if definition.spell.as_ref().is_some_and(|p| {
        p.instructions.iter().any(|i| {
            matches!(
                i,
                Instruction::Scry { .. }
                    | Instruction::Surveil { .. }
                    | kernel!(Mill {
                        amount: fixed!(_),
                        ..
                    })
            )
        })
    }) {
        controller_library::validate_source_shape(&canonical_abilities)?;
    }
    if let Some(p) = definition.spell.as_ref()
        && temporary_permanent_keyword_program(p).is_some()
    {
        validate_temporary_permanent_bindings(
            &binding,
            &definition.oracle_id,
            &abilities,
            &canonical_abilities,
        )?;
    }
    if definition.spell.as_ref().is_some_and(|p| {
        mtg_contracts::controller_scry_program(p)
            || mtg_contracts::controller_surveil_program(p)
            || mtg_contracts::fixed_mill_program(p)
    }) {
        controller_library::validate_bindings(
            &binding,
            &definition.oracle_id,
            &canonical_abilities,
        )?;
    }
    if definition
        .spell
        .as_ref()
        .is_some_and(mtg_contracts::controller_library_selection_program)
    {
        library_selection::validate_bindings(
            &binding,
            &definition.oracle_id,
            &canonical_abilities,
        )?;
    }
    if matches!(
        definition.characteristics.kind,
        CardKind::Instant | CardKind::Sorcery
    ) && binding["nodes"].as_array().is_some_and(|nodes| {
        nodes
            .iter()
            .any(|n| n["node"] == "event" && n["operation"] == "discard")
    }) && group_discard.is_none()
        && discard_target.is_none()
        && !announced_x_effects
        && !matches!(abilities.as_slice(), [Ability::ComposedSpell(_)])
        && !definition.spell.as_ref().is_some_and(|p| {
            controller_draw_discard_program(p)
                || matches!(
                    p.instructions.as_slice(),
                    [
                        kernel!(Draw {
                            player: kir::PlayerExpr::Is { .. },
                            amount: announced_x!()
                        }),
                        kernel!(Discard {
                            player: you!() | event_player!(),
                            amount: fixed!(_),
                            random: false
                        })
                    ]
                )
        })
    {
        return Err(unsupported(
            "binding",
            "ordinary discard binding requires a complete supported discard spell",
        ));
    }
    if definition
        .spell
        .as_ref()
        .is_some_and(controller_draw_discard_program)
    {
        draw_discard::validate_source(
            &shape,
            &binding,
            &definition.oracle_id,
            &canonical_abilities,
        )?;
    }
    if definition
        .spell
        .as_ref()
        .is_some_and(controller_hand_return_program)
    {
        hand_return::validate_source(&binding, &definition.oracle_id, &canonical_abilities)?;
    }
    if definition
        .spell
        .as_ref()
        .is_some_and(controller_library_order_program)
    {
        library_order::validate_source(&binding, &definition.oracle_id, &canonical_abilities)?;
    }
    if definition
        .spell
        .as_ref()
        .is_some_and(controller_counted_draw_program)
    {
        controller_counted_draw::validate_source(
            &binding,
            &definition.oracle_id,
            &canonical_abilities,
        )?;
    }
    if definition
        .spell
        .as_ref()
        .is_some_and(targeted_counted_draw_program)
    {
        targeted_counted_draw::validate_source(
            &binding,
            &definition.oracle_id,
            &canonical_abilities,
        )?;
    }
    if definition
        .activated
        .iter()
        .any(mtg_contracts::activated_artifact_draw_ability)
    {
        activated_artifact_draw::validate_source(
            &binding,
            &definition.oracle_id,
            result.abilities.get(),
        )?;
    }
    paid_counted_draw::validate_source(&definition, &labels, &binding, result.abilities.get())?;
    if definition.static_abilities.iter().any(|ability| {
        matches!(
            ability.effect,
            StaticEffect::UnlimitedControllerHandSize
                | StaticEffect::ReduceMaximumHandSize { .. }
                | StaticEffect::IncreaseMaximumHandSize { .. }
        )
    }) {
        unlimited_hand_size::validate_source(
            &definition,
            &labels,
            &binding,
            result.abilities.get(),
        )?;
    }
    controller_creature_pump::bind(&definition, &shape, &binding, &result)?;
    definition.capabilities = definition.requirements();
    // The commander choice requires all three source labels at admission.
    // Its shape and binding graph were checked before lowering.
    if !definition.spell.as_ref().is_some_and(commander_choose_both) {
        definition
            .validate()
            .map_err(|e| unsupported("definition", e))?;
    }
    if definition
        .capabilities
        .contains(&Capability::CastManaValueSweepV1)
    {
        cast_mana_value_sweep::validate_binding(&definition.oracle_id, &shape, &binding)?;
    }
    if definition
        .capabilities
        .contains(&Capability::ControlledCreatureSpellCopyV1)
    {
        controlled_creature_spell_copy::validate_binding(&definition, &shape, &binding)?;
    }
    if definition.capabilities.contains(&Capability::DemonstrateV1) {
        demonstrate::validate_binding(&definition.oracle_id, &shape, &binding)?;
    }
    self_spell_protection::validate_binding(&definition, &binding)?;
    cast_entry::validate_binding(&definition, &shape, &binding)?;
    source_regeneration::bind(&definition, &shape, &binding)?;
    activated_creature_tap::bind(&definition, &shape, &binding)?;
    own_graveyard_card_return::validate_binding(&definition, &shape, &binding)?;
    power_entry_draw::validate_source(&definition, &labels, &original, &shape, &binding)?;
    graveyard_battlefield_return::validate_binding(&definition, &shape, &binding)?;
    creature_entry_return::validate_source(&definition, &labels, &original, &shape, &binding)?;
    entry_graveyard_exile::validate_binding(&definition, &shape, &binding)?;
    land_entry_gain_life::validate_binding(&definition, &shape, &binding)?;
    land_entry_scry::validate_binding(&definition, &shape, &binding)?;
    if !land_reclamation {
        controlled_land_entry::bind(&definition, &shape, &binding)?;
    }
    other_land_entry::validate_binding(&definition, &shape, &binding)?;
    reveal_land_entry::validate_binding(&definition, &shape, &binding)?;
    checkland_entry::validate_binding(&definition, &shape, &binding)?;
    land_sacrifice_draw::validate_binding(&definition, &shape, &binding)?;
    combat_hand_cast::bind(&definition, &shape, &binding)?;
    if let Some(spell) = &definition.spell
        && !spell.modes.is_empty()
    {
        // The complete standalone choose-one lowering above binds option order.
        // Reject an unlabelled source rather than inventing mode descriptions.
        labels.spell_modes = labels
            .blocks
            .iter()
            .flat_map(|block| block.lines().skip(1))
            .map(str::to_owned)
            .collect();
        if commander_choose_both(spell) {
            labels.spell_modes.push(labels.spell_modes.join("\n"));
        }
        if labels.spell_modes.len() != spell.modes.len()
            || labels.spell_modes.iter().any(|line| !line.starts_with('•'))
        {
            return Err(unsupported(
                "labels.modes",
                "source option labels do not match compiled modes",
            ));
        }
    }
    if definition
        .capabilities
        .contains(&Capability::OpeningPlacementV1)
    {
        let mut graph = bindings::Bindings::new(&definition.oracle_id, &shape);
        let p = "/faces/0/abilities/0";
        graph.ability(p, "static", "static-source-controller");
        let ast = opening_composition
            .clone()
            .or_else(|| opening_placement_ast(&definition.name, &source.oracle_text))
            .ok_or_else(|| unsupported("source", "incomplete opening action"))?;
        graph.reference(
            &format!("{p}/effect/what"),
            &ast[0]["effect"]["what"],
            &graph.id("/source", "object"),
            "201.5",
        );
        graph.reference(
            &format!("{p}/effect/who"),
            &serde_json::json!({"player":"you"}),
            &graph.id(p, "you"),
            "109.5",
        );
        if opening_composition.is_some() {
            for i in [1, 2] {
                let ability = format!("/faces/0/abilities/{i}");
                controller_land_types::bind_grant(&mut graph, &ability, &ast[i]["effect"]["what"]);
            }
        }
        graph.validate_ordered(
            &binding,
            "opening placement requires owner and exact source",
        )?;
    }
    if opening_composition.is_none() {
        controller_land_types::bind(&definition, &shape, &binding)?;
    }
    definition.source_labels = Some(labels);
    definition
        .validate()
        .map_err(|e| unsupported("definition", e))?;
    let entry_copy = definition
        .static_abilities
        .iter()
        .any(|ability| ability.effect == StaticEffect::EnterAsCopyOfPermanentEnteredThisTurn);
    if (self_graveyard_shuffle || entry_copy || archive.manifest_file == entry_copy::MANIFEST_FILE)
        && !archive
            .record_sha256
            .contains(&sha256(record.trim_end().as_bytes()))
    {
        if self_graveyard_shuffle {
            return Err(unsupported(
                "record",
                "self graveyard replacement requires its accepted exact archive row",
            ));
        }
        return Err(invalid("record", "unregistered entry-copy archive row"));
    }
    Ok(definition)
}

/// The one reviewed residual of the look-top mana source: the look-top step
/// of the mana ability's block, which its typed definition lowers. Every
/// residual and every unaccounted step must be exactly that one, at that
/// location; any other, card-level or in any block, rejects the source.
pub(crate) fn reviewed_look_top_residual(
    resolution: &resolution::Resolution,
    printed: &serde_json::Value,
) -> Result<(), Failure> {
    const BLOCK: usize = 1;
    const STEP: &str = "/effects/0/steps/0";
    const EFFECT: &str = "look-top-may-reveal-for-mana";
    const REVIEWED: &str = "step:look-top-may-reveal-for-mana";
    let residuals = resolution.residuals();
    if residuals != [(Some(BLOCK), REVIEWED)] {
        let other = residuals
            .iter()
            .find(|(_, r)| *r != REVIEWED)
            .map_or(REVIEWED, |(_, r)| *r);
        return Err(unsupported("binding", other));
    }
    let unaccounted = resolution
        .unaccounted_steps(printed)
        .map_err(|e| unsupported("binding", e.reason))?;
    if unaccounted != [(BLOCK, STEP.to_string(), EFFECT.to_string())] {
        return Err(unsupported(
            "binding",
            "look-top mana source has an unreviewed unaccounted step",
        ));
    }
    Ok(())
}
