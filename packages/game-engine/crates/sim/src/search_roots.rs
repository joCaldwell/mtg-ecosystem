//! Trusted fixture preparation for the predeclared private-information workload.
//! Preparation is outside search timing; the live root still counts toward RSS.
//! Every policy choice below uses only that chooser's current Observation.
use super::*;
use mtg_contracts::CardKind;
use mtg_core::{MatchConfig, PlayerRequest, PlayerResponse};

pub(super) const WORKLOAD: &str = "commander-open-search-v1";
pub(super) const ROOT_COUNT: usize = 12;
const ROOT_SEED_DOMAIN: &str = "commander-open-search-v1/root/";
const GENERATOR: &str = "foundations-land-only-roots-v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RootCase {
    pub index: usize,
    pub config: MatchConfig,
    pub viewer: PlayerId,
    pub mulligans: [u8; 4],
    pub departures_before_setup: Vec<PlayerId>,
    pub viewer_precombat_main: usize,
    pub capture_after_land_attempt: bool,
    pub generator: String,
}
impl RootCase {
    pub fn named(index: usize) -> Result<Self, String> {
        if index >= ROOT_COUNT {
            return Err("root index outside frozen twelve-case workload".into());
        }
        let digest = sha256(format!("{ROOT_SEED_DOMAIN}{index}").as_bytes());
        let seed = std::array::from_fn(|i| {
            u8::from_str_radix(&digest[i * 2..i * 2 + 2], 16).expect("SHA256 hex")
        });
        let viewer = PlayerId((index % 4) as u8);
        let seat = viewer.0;
        let departures_before_setup = match index / 4 {
            0 => vec![],
            1 => vec![PlayerId((seat + 2) % 4)],
            2 => vec![PlayerId((seat + 1) % 4), PlayerId((seat + 3) % 4)],
            _ => unreachable!(),
        };
        Ok(Self {
            index,
            config: MatchConfig::foundations(seed, viewer),
            viewer,
            mulligans: std::array::from_fn(|seat| ((index / 4 + seat) % 4) as u8),
            departures_before_setup,
            viewer_precombat_main: if index / 4 == 1 { 6 } else { 2 },
            capture_after_land_attempt: index / 4 == 1,
            generator: GENERATOR.into(),
        })
    }
}

pub(super) struct RootFixture {
    pub case: RootCase,
    pub game: Game,
    pub land_played_at_capture: bool,
}

pub(super) fn capture(game: &Game, viewer: PlayerId) -> Result<mtg_core::SearchCapture, String> {
    let mut outcome = game.capture_search(viewer, Budget::new(1024));
    for _ in 0..100_000 {
        match outcome {
            mtg_core::CaptureOutcome::Complete(capture) => return Ok(capture),
            mtg_core::CaptureOutcome::Rejected(reason) => {
                return Err(format!("frozen root capture rejected: {reason:?}"));
            }
            mtg_core::CaptureOutcome::Incomplete { work, .. } => {
                outcome = game.resume_search_capture(work, Budget::new(1024));
            }
        }
    }
    Err("frozen root capture work limit; no substitute root selected".into())
}

fn basic_land(catalog: &Catalog, o: &Observation) -> Option<mtg_core::ObjectHandle> {
    let view = &o.view;
    if view.active != Some(view.viewer)
        || view.turn.step != TurnStep::PrecombatMain
        || view.turn.lands_played != 0
    {
        return None;
    }
    view.objects.iter().find_map(|object| {
        (object.owner == view.viewer && object.zone == (Zone::Hand { owner: view.viewer }))
            .then(|| {
                catalog.definitions.iter().find(|definition| {
                    definition.oracle_id == object.oracle_id
                        && definition.characteristics.kind == CardKind::Land
                        && definition.characteristics.basic
                })
            })
            .flatten()
            .map(|_| object.handle.clone())
    })
}

fn choose(
    catalog: &Catalog,
    case: &RootCase,
    observation: &Observation,
) -> Result<PlayerResponse, String> {
    let request = observation
        .decision
        .as_ref()
        .ok_or("root generator missing decision")?;
    Ok(match request {
        PlayerRequest::Mulligan {
            player,
            mulligans_taken,
            can_mulligan,
            ..
        } => PlayerResponse::Mulligan {
            take: *can_mulligan && *mulligans_taken < case.mulligans[player.0 as usize],
        },
        PlayerRequest::BottomOpening {
            count, candidates, ..
        } => PlayerResponse::BottomOpening {
            objects: candidates.iter().rev().take(*count).cloned().collect(),
        },
        PlayerRequest::Discard {
            count, candidates, ..
        } => PlayerResponse::Discard {
            objects: candidates.iter().rev().take(*count).cloned().collect(),
        },
        PlayerRequest::Attackers { .. } => PlayerResponse::Attackers { attacks: vec![] },
        PlayerRequest::Blockers { .. } => PlayerResponse::Blockers { blocks: vec![] },
        PlayerRequest::CommanderReturn { .. } => {
            PlayerResponse::CommanderReturn { to_command: true }
        }
        PlayerRequest::Priority { .. } => basic_land(catalog, observation)
            .map_or(PlayerResponse::Pass, |object| PlayerResponse::PlayLand {
                object,
            }),
        _ => {
            return Err(format!(
                "unsupported frozen root-generator request: {request:?}"
            ));
        }
    })
}

pub(super) fn prepare(catalog: &Catalog, index: usize) -> Result<RootFixture, String> {
    let case = RootCase::named(index)?;
    let prepared = Prepared::admit_pod(catalog.clone(), case.config.clone())
        .map_err(|e| format!("root admission: {e:?}"))?;
    let mut game = Game::with_trace(prepared, TraceMode::Off);
    for player in &case.departures_before_setup {
        game.concede(*player)?;
    }
    let mut last_viewer_main = None;
    let mut viewer_mains = 0usize;
    let mut target_land_attempted = false;
    // These fixture caps are diagnostics, never a substitute seed or a rollout.
    for _ in 0..100_000 {
        let request = match game.advance(Budget::new(1024)) {
            Advance::Suspended { .. } => continue,
            Advance::AwaitingDecision(request) => request,
            other => {
                return Err(format!(
                    "root preparation stopped before capture: {other:?}"
                ));
            }
        };
        let observation = super::observation(&game, request.player())?;
        let is_viewer_main = observation.view.viewer == case.viewer
            && observation.view.active == Some(case.viewer)
            && observation.view.turn.step == TurnStep::PrecombatMain
            && matches!(observation.decision, Some(PlayerRequest::Priority { .. }));
        if is_viewer_main {
            if last_viewer_main != Some(observation.view.turn.number) {
                last_viewer_main = Some(observation.view.turn.number);
                viewer_mains += 1;
            }
            if viewer_mains == case.viewer_precombat_main {
                if !case.capture_after_land_attempt
                    || target_land_attempted
                    || basic_land(catalog, &observation).is_none()
                {
                    return Ok(RootFixture {
                        case,
                        game,
                        land_played_at_capture: target_land_attempted,
                    });
                }
                target_land_attempted = true;
            }
        }
        let response = choose(catalog, &case, &observation)?;
        let token = observation
            .decision
            .as_ref()
            .ok_or("root decision disappeared")?
            .token()
            .clone();
        let receipt = match game.check_player_response(
            request.player(),
            token,
            response,
            Budget::unlimited(),
        ) {
            QueryResult::Complete(QueryValue::Validated(receipt)) => receipt,
            QueryResult::Rejected(reason) => {
                return Err(format!("root response rejected: {reason:?}"));
            }
            _ => return Err("root response validation did not complete".into()),
        };
        game.submit(receipt)?;
    }
    Err("frozen root-generator host limit; no substitute root selected".into())
}
