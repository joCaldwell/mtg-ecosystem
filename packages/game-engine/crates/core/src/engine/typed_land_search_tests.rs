//! Complete frozen paid sources, private legal 100-card setup, no admission.
//! CR 205.3i, 305.6, 401.2, 701.23b, 701.24a/e (2026-09-25):
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;
const SOURCES: [(&str, &str, usize, usize, bool); 3] = [
    ("Nature's Lore",include_str!("../../../../tests/fixtures/ramos/78826359-fe63-44ad-adc4-a17ffcd710e4.json"),2,1,false),
    ("Farseek",include_str!("../../../../tests/fixtures/ramos/495e52e6-4c2b-4574-9474-eadbdcc8b4ac.json"),2,1,true),
    ("Skyshroud Claim",include_str!("../../../../tests/fixtures/ramos/376c9d3f-21d3-4251-bb6a-026fa9e1b0e1.json"),4,2,false),
];
fn source(raw: &str, manifest: &str) -> Definition {
    let result = mtg_compiler::compile(manifest,raw);
    let mtg_compiler::CompileResult::Lowered { definition } = result else { panic!("{result:?}") };
    *definition
}
fn typed_prepared(raw: &str) -> Prepared {
    let mut p = search_prepared("Rampant Growth");
    let mut opening_probe = Game::new(p.clone());
    opening(&mut opening_probe);
    let opening_cards: BTreeSet<_> = opening_probe.state.rules.objects.values()
        .filter(|o| o.zone == (Zone::Hand { owner: PlayerId(0) }))
        .map(|o| o.id.card).collect();
    let mut c = p.catalog.as_ref().clone();
    let manifest = include_str!("../../../../tests/fixtures/ramos/archive-manifest-current.json");
    let spell = source(raw,manifest);
    let new = spell.oracle_id.clone();
    let old = c.definitions.iter().find(|d| d.name == "Rampant Growth").unwrap().oracle_id.clone();
    let ramos = source(include_str!("../../../../tests/fixtures/ramos/3ed41d2d-211b-4013-8562-8c64d54cc43a.json"),manifest);
    let commander = ramos.oracle_id.clone();
    let tower = source(include_str!("../../../../tests/fixtures/command-tower.json"),mtg_compiler::fixtures::MANIFEST);
    let tower_id = tower.oracle_id.clone();
    c.definitions.extend([spell,ramos,tower]);
    c.definitions.sort_by(|a,b| a.oracle_id.cmp(&b.oracle_id));
    c.validate().unwrap();
    let basics: Vec<_> = ["Plains","Island","Swamp","Mountain"].map(|name| c.definitions.iter().find(|d| d.name == name).unwrap().oracle_id.clone()).into();
    let mut n = 0;
    for card in &mut p.config.cards {
        if card.oracle_id == old { card.oracle_id = new.clone(); }
        if card.owner == PlayerId(0) {
            if matches!(card.zone,Zone::Command { .. }) { card.oracle_id = commander.clone(); }
            // Only original library rows change: the seeded opening keeps its
            // ordinary Forest payment sources. Ramos makes all five colors legal.
            if matches!(card.zone,Zone::Library { .. }) && !opening_cards.contains(&card.id) && n <= 20 {
                card.oracle_id = if n == 20 { tower_id.clone() } else { basics[n % 4].clone() };
                n += 1;
            }
        }
    }
    let config = p.match_config.as_mut().unwrap();
    config.decks[0].commander = commander;
    let mut counts = BTreeMap::<String,u32>::new();
    for card in p.config.cards.iter().filter(|c| c.owner == PlayerId(0)) { *counts.entry(card.oracle_id.clone()).or_default() += 1; }
    config.decks[0].cards = counts.into_iter().map(|(oracle_id,count)| DeckEntry {oracle_id,count}).collect();
    assert_eq!(config.decks[0].cards.iter().map(|r| r.count).sum::<u32>(),100);
    p.config.profile = "typed-land-search-private-source-witness".into();
    p.catalog_digest = digest(&c).unwrap();
    p.profile_digest = digest(&(&p.config,&p.match_config)).unwrap();
    p.catalog = Arc::new(c);
    p
}
fn paid_search(g: &mut Game, name: &str, cost: usize) -> Request {
    let spell = develop_search(g,name,cost);
    announce(g,spell.0,vec![]);
    let spell = g.state.rules.stack.last().unwrap();
    assert_eq!(spell.paid.as_ref(),Some(&spell.total_cost));
    assert_eq!(spell.total_cost.generic,Quantity::from((cost-1) as i64));
    assert_eq!(spell.total_cost.colored,vec![Color::G]);
    for _ in 0..20 {
        let r = request(g);
        if matches!(r,Request::SearchLands { .. }) { return r; }
        g.respond(r.player(),r.decision_id(),Response::Pass).unwrap();
    }
    panic!("search not reached")
}
fn validate(g: &Game, response: PlayerResponse) -> ValidatedResponse {
    let before = g.state.clone();
    let trace = g.trace.clone();
    let token = observation(g,0).decision.unwrap().token().clone();
    let QueryResult::Incomplete { work,.. } = g.check_player_response(PlayerId(0),token.clone(),response.clone(),Budget::new(1)) else { panic!("bounded validation") };
    let work = g.restore_work(&g.save_work(&work).unwrap()).unwrap();
    let QueryResult::Complete(QueryValue::Validated(receipt)) = g.resume_work(work,Budget::unlimited()) else { panic!("resumed validation") };
    assert!(matches!(g.check_player_response(PlayerId(0),token,response,Budget::unlimited()),QueryResult::Complete(QueryValue::Validated(_))));
    assert_eq!(g.state,before);
    assert_eq!(g.trace,trace);
    receipt
}
fn observation_work(g: &Game, viewer: PlayerId) {
    let before = g.state.clone();
    let trace = g.trace.clone();
    let QueryResult::Incomplete { work,.. } = g.observe(viewer,Budget::new(1)) else { panic!("bounded observation") };
    let saved = g.save_work(&work).unwrap();
    let bad: serde_json::Value = serde_json::from_str(&saved).unwrap();
    for key in ["build_id","catalog_digest","profile_digest"] {
        if bad.get(key).is_some() { let mut forged=bad.clone();forged[key]="foreign".into();assert!(g.restore_work(&forged.to_string()).is_err()); }
    }
    let work = g.restore_work(&saved).unwrap();
    let QueryResult::Complete(QueryValue::Observation(own)) = g.resume_work(work,Budget::unlimited()) else { panic!("resumed observation") };
    assert_eq!(own,observation(g,viewer.0));
    assert_eq!(g.state,before);
    assert_eq!(g.trace,trace);
}
#[test]
fn typed_land_search_paid_sources_zero_one_two_private_query_restore_replay() {
    for (name,raw,cost,maximum,tapped) in SOURCES {
        let p = typed_prepared(raw);
        for count in 0..=maximum {
            let order = std::cell::RefCell::new(None);
            for &reverse in if count==2 { &[false,true][..] } else { &[false][..] } {
                traces(p.clone(),|g| {
                    let r = paid_search(g,name,cost);
                    let Request::SearchLands { candidates,maximum: offered,.. } = &r else { unreachable!() };
                    assert_eq!(*offered,maximum as u32);
                    check(g);
                    let Some(PlayerRequest::SearchLands { candidates: visible,.. }) = observation(g,0).decision else { panic!("private candidates") };
                    let shown: BTreeSet<_> = visible.iter().map(|v| g.definition(g.resolve_search_handle(PlayerId(0),&v.handle).unwrap().card).unwrap().name.as_str()).collect();
                    if name=="Farseek" { assert_eq!(shown,BTreeSet::from(["Plains","Island","Swamp","Mountain"])); }
                    else { assert_eq!(shown,BTreeSet::from(["Forest"])); }
                    assert!(!shown.contains("Command Tower"));
                    // Supplemental synthetic library-order probe only; natural
                    // paid path below owns replay and privacy evidence.
                    let mut reordered = g.fork();
                    reordered.state.rules.libraries.get_mut(&PlayerId(0)).unwrap().reverse();
                    assert_eq!(observation(g,0),observation(&reordered,0));
                    for seat in 0..4 {
                        observation_work(g,PlayerId(seat));
                        let own = observation(g,seat);
                        assert!(own.view.objects.iter().all(|o| !matches!(o.zone,Zone::Library { .. })));
                        if seat!=0 { assert!(own.decision.is_none());assert_eq!(own,observation(&reordered,seat)); }
                    }
                    let before = g.state.clone();
                    let wrong = g.state.rules.objects.values().find(|o| o.zone==(Zone::Library {owner:PlayerId(0)}) && !candidates.contains(&o.id)).unwrap().id;
                    let enemy = g.state.rules.objects.values().find(|o| o.zone==(Zone::Library {owner:PlayerId(1)})).unwrap().id;
                    for (actor,objects) in [(PlayerId(1),vec![]),(PlayerId(0),vec![candidates[0];2]),(PlayerId(0),candidates[..maximum+1].to_vec()),(PlayerId(0),vec![wrong]),(PlayerId(0),vec![enemy]),(PlayerId(0),vec![g.state.rules.stack.last().unwrap().source]),(PlayerId(0),vec![ObjectId {card:candidates[0].card,incarnation:candidates[0].incarnation+1}])] {
                        assert!(g.respond(actor,r.decision_id(),Response::SearchLands {objects}).is_err());assert_eq!(g.state,before);
                    }
                    for handle in [ObjectHandle("foreign:search:0".into()),ObjectHandle(format!("{}0",visible[0].handle.0))] {
                        assert!(g.resolve_search_handle(PlayerId(0),&handle).is_err());
                    }
                    for seat in 1..4 { assert!(g.resolve_search_handle(PlayerId(seat),&visible[0].handle).is_err()); }
                    for mutation in 0..5 {
                        let mut forged = g.fork();
                        let Frame::ResolutionChoice {frame,choice:ResolutionChoice::Search {maximum,split,source_block,..}}=&mut forged.state.rules.frame else {unreachable!()};
                        match mutation {0=>*maximum+=1,1=>*split=!*split,2=>*source_block+=1,3=>frame.cursor+=1,_=>frame.spell.controller=PlayerId(1)};
                        assert!(Game::restore(g.prepared.as_ref().clone(),&forged.snapshot().unwrap()).is_err());
                    }
                    let selected=candidates[..count].to_vec();
                    let mut handles=visible[..count].iter().map(|c|c.handle.clone()).collect::<Vec<_>>();
                    if reverse {handles.reverse();}
                    let receipt=validate(g,PlayerResponse::SearchLands {objects:handles.clone()});
                    let stale=validate(g,PlayerResponse::SearchLands {objects:handles.clone()});
                    g.submit(receipt).unwrap();
                    check(g);
                    let library=g.state.rules.libraries[&PlayerId(0)].len();
                    let _=g.advance(Budget::new(1));
                    assert!(matches!(g.state.rules.frame,Frame::ResolutionChoice {choice:ResolutionChoice::Shuffle {..},..}));
                    check(g);
                    let before_stale=g.state.clone();
                    assert!(g.submit(stale).is_err());
                    assert_eq!(g.state,before_stale);
                    for id in selected {
                        let o=g.current(g.object_id(id.card).unwrap()).unwrap();
                        assert_eq!(o.zone,Zone::Battlefield);assert_eq!(o.tapped,tapped);assert_eq!(o.controller,PlayerId(0));assert_ne!(o.id,id);
                    }
                    for handle in handles {assert!(g.resolve_search_handle(PlayerId(0),&handle).is_err());}
                    // Retain/restore every actual shuffle word boundary; one
                    // independent replay at each such boundary in Detailed mode.
                    while matches!(g.state.rules.frame,Frame::ResolutionChoice {choice:ResolutionChoice::Shuffle {..},..}) {
                        let restored=Game::restore(g.prepared.as_ref().clone(),&g.snapshot().unwrap()).unwrap();assert_eq!(restored.state,g.state);
                        if g.trace.mode==TraceMode::Detailed {check(g);}
                        let _=g.advance(Budget::new(1));
                    }
                    resolve(g,vec![]);
                    check(g);
                    assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(),library-count);
                    for seat in 0..4 {
                        assert_eq!(observation(g,seat).view.players[0].library_shuffles,1);
                        assert!(g.state.knowledge.visibility[seat as usize].history.iter().all(|v|v.to_public().objects.iter().all(|o|!matches!(o.zone,Zone::Library {..}))));
                    }
                    if let Some(old)=order.borrow().as_ref(){assert_eq!(old,&g.state.rules);}
                    *order.borrow_mut()=Some(g.state.rules.clone());
                    eprintln!("typed_land_search name={name:?} selected={count} reverse={reverse} trace={:?} build={} boundary={} journal={} state={}",g.trace.mode,BUILD_ID,g.rule_boundary(),digest(&g.replay_journal()).unwrap(),g.state_digest().unwrap());
                });
            }
        }
    }
}

#[test]
fn typed_land_search_synthetic_generic_maximum_and_small_shuffle_controls() {
    // Synthetic exact descriptor counts over complete spell sources. No new
    // Oracle source or admitted pool is asserted by these supplemental controls.
    for (quantity,offered,count) in [("3",3,3),("999999999999999999999999999999",100,3)] {
        let mut p=typed_prepared(SOURCES[2].1);
        let mut c=p.catalog.as_ref().clone();
        let d=c.definitions.iter_mut().find(|d|d.name=="Skyshroud Claim").unwrap();
        let Instruction::SearchTypedLands {search,..}=&mut d.spell.as_mut().unwrap().instructions[0] else {unreachable!()};
        search.maximum=quantity.parse().unwrap();c.validate().unwrap();p.catalog_digest=digest(&c).unwrap();p.catalog=Arc::new(c);
        let mut g=Game::new(p);
        let r=paid_search(&mut g,"Skyshroud Claim",4);
        let Request::SearchLands {candidates,maximum,..}=&r else {unreachable!()};
        assert_eq!(*maximum,offered);
        g.respond(r.player(),r.decision_id(),Response::SearchLands {objects:candidates[..count].to_vec()}).unwrap();
        resolve(&mut g,vec![]);check(&g);
    }
    let mut base=Game::new(typed_prepared(SOURCES[0].1));
    let _=paid_search(&mut base,"Nature's Lore",2);
    for count in 0..=1 { for remainder in 0..=1 {
        // Synthetic fork only: shrink the actual paid path's library through
        // ordinary movement; this is not an accepted-input reachability proof.
        let mut g=base.fork();
        let found=g.search_lands(PlayerId(0))[..count].to_vec();
        let Frame::ResolutionChoice {frame,..}=g.state.rules.frame.clone() else {unreachable!()};
        let cause=BatchCause::Instruction {stack_id:frame.spell.id,player:PlayerId(0),source:frame.spell.source,cursor:frame.cursor,source_block:0};
        let library=g.state.rules.objects.values().filter(|o|o.zone==(Zone::Library {owner:PlayerId(0)})&&!found.contains(&o.id)).map(|o|o.id).collect::<Vec<_>>();
        for id in &library[remainder..] {g.move_object_for(*id,Zone::Graveyard {owner:PlayerId(0)},cause.clone()).unwrap();}
        g.refresh_visibility().unwrap();
        let random=g.state.rules.match_state.as_ref().unwrap().random.word_position;
        let r=request(&mut g);
        g.respond(r.player(),r.decision_id(),Response::SearchLands {objects:found}).unwrap();
        resolve(&mut g,vec![]);
        assert_eq!(g.state.rules.libraries[&PlayerId(0)].len(),remainder);
        assert_eq!(g.state.rules.match_state.as_ref().unwrap().random.word_position,random);
        assert_eq!(observation(&g,0).view.players[0].library_shuffles,1);
    }}
}
#[test]
fn typed_land_search_capability_stays_outside_reviewed_consumers() {
    use crate::profile_data::{Feature,ProfileError};
    let p=typed_prepared(SOURCES[0].1);
    assert!(Prepared::admit_pod(p.catalog.as_ref().clone(),p.match_config.clone().unwrap()).is_err());
    for feature in [Feature::Hosting,Feature::Search] {
        let mut profile=match_profile("commander-foundations-v1").unwrap().clone();
        profile.hosted=feature==Feature::Hosting;profile.search=feature==Feature::Search;
        assert!(matches!(profile.validate_requested_features(&p.catalog),Err(ProfileError::UnreviewedCapabilities {capabilities,..}) if capabilities.contains(&Capability::TypedLandSearchV1)));
    }
}

#[test]
fn typed_land_search_paid_farseek_each_alternative_enters_tapped() {
    let p=typed_prepared(SOURCES[1].1);
    for name in ["Plains","Island","Swamp","Mountain"] {
        traces(p.clone(),|g| {
            let r=paid_search(g,"Farseek",2);
            let Request::SearchLands {candidates,..}=&r else {unreachable!()};
            let id=*candidates.iter().find(|o|g.definition(o.card).unwrap().name==name).unwrap();
            g.respond(r.player(),r.decision_id(),Response::SearchLands {objects:vec![id]}).unwrap();
            resolve(g,vec![]);check(g);
            let o=g.current(g.object_id(id.card).unwrap()).unwrap();
            assert_eq!(o.zone,Zone::Battlefield);assert!(o.tapped);
        });
    }
}
