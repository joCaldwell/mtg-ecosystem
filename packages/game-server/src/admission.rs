//! Read-only, exact-build admission bridge for the product coordinator.
//! This command creates no registry, seed, seat, credential or consumer.
use mtg_core::{Decklist, Feature, MatchConfig, PlayerId, match_profile, validate_pod};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

pub const MAX_ADMISSION_BYTES: usize = 65_536;
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionRequest {
    pub version: u32,
    pub operation: String,
    pub profile: String,
    pub engine_build_id: String,
    pub server_build_id: String,
    pub catalog_sha256: String,
    pub deck: Decklist,
}
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AdmissionResponse {
    Admitted {
        version: u32,
        profile: String,
        engine_build_id: String,
        server_build_id: String,
        catalog_sha256: String,
        deck: Decklist,
    },
    Rejected {
        version: u32,
        reason: &'static str,
        detail: &'static str,
    },
}
fn rejected(reason: &'static str, detail: &'static str) -> AdmissionResponse {
    AdmissionResponse::Rejected {
        version: 1,
        reason,
        detail,
    }
}
pub fn admit(request: AdmissionRequest) -> AdmissionResponse {
    if request.version != 1 || request.operation != "admit" {
        return rejected("identity", "unsupported admission protocol");
    }
    let Some(profile) = match_profile(&request.profile) else {
        return rejected("profile", "unknown profile");
    };
    if request.engine_build_id != crate::ENGINE_BUILD.id
        || request.server_build_id != *crate::SERVER_BUILD_ID
        || request.catalog_sha256 != profile.catalog_sha256
    {
        return rejected("identity", "producer or catalog identity differs");
    }
    if profile
        .require_feature(profile.catalog(), Feature::Hosting)
        .is_err()
    {
        return rejected("profile", "profile is not reviewed for hosting");
    }
    // Zero is only a deterministic validation witness. It never provisions a game
    // and neither this witness nor any resulting private pod digest is exported.
    let mut config = MatchConfig::for_profile(&request.profile, [0; 32], PlayerId(0))
        .expect("looked-up profile");
    let unique = request
        .deck
        .cards
        .iter()
        .map(|row| &row.oracle_id)
        .collect::<std::collections::BTreeSet<_>>();
    if request.deck.cards.len() > 100 || unique.len() != request.deck.cards.len() {
        return rejected("deck", "deck entries must use unique Oracle IDs");
    }
    config.decks[0] = request.deck;
    let Ok(pod) = validate_pod(profile.catalog(), &config) else {
        return rejected("deck", "deck is not admitted by the exact hosted profile");
    };
    AdmissionResponse::Admitted {
        version: 1,
        profile: profile.name.clone(),
        engine_build_id: crate::ENGINE_BUILD.id.into(),
        server_build_id: crate::SERVER_BUILD_ID.clone(),
        catalog_sha256: profile.catalog_sha256.clone(),
        deck: pod.normalized_config().decks[0].clone(),
    }
}
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(MAX_ADMISSION_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_ADMISSION_BYTES {
        return Err("admission input exceeds bound".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "invalid admission encoding")?;
    // Duplicate-aware and strict, including nested fields/count spellings.
    let request: AdmissionRequest =
        crate::codec::exact(text).map_err(|_| "invalid admission request")?;
    let response = serde_json::to_vec(&admit(request))?;
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(&response)?;
    stdout.write_all(b"\n")?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request(profile: &str) -> AdmissionRequest {
        let p = match_profile(profile).unwrap();
        AdmissionRequest {
            version: 1,
            operation: "admit".into(),
            profile: profile.into(),
            engine_build_id: crate::ENGINE_BUILD.id.into(),
            server_build_id: crate::SERVER_BUILD_ID.clone(),
            catalog_sha256: p.catalog_sha256.clone(),
            deck: p.decks[0].clone(),
        }
    }
    #[test]
    fn exact_hosted_decks_and_identity_fences() {
        for profile in ["commander-foundations-v1", "commander-attachments-v1"] {
            assert!(matches!(
                admit(request(profile)),
                AdmissionResponse::Admitted { .. }
            ));
            let mut changed = request(profile);
            changed.engine_build_id.push('0');
            assert!(matches!(
                admit(changed),
                AdmissionResponse::Rejected {
                    reason: "identity",
                    ..
                }
            ));
            let mut changed = request(profile);
            changed.deck.cards[0].count = 0;
            assert!(matches!(
                admit(changed),
                AdmissionResponse::Rejected { reason: "deck", .. }
            ));
            let mut changed = request(profile);
            changed.deck.commander = "unknown".into();
            assert!(matches!(
                admit(changed),
                AdmissionResponse::Rejected { reason: "deck", .. }
            ));
        }
    }
    #[test]
    fn strict_request_rejects_duplicate_unknown_and_imprecise_counts() {
        let p = match_profile("commander-foundations-v1").unwrap();
        let raw = serde_json::json!({"version":1,"operation":"admit","profile":p.name,
            "engine_build_id":crate::ENGINE_BUILD.id,"server_build_id":*crate::SERVER_BUILD_ID,
            "catalog_sha256":p.catalog_sha256,"deck":p.decks[0]})
        .to_string();
        assert!(crate::codec::exact::<AdmissionRequest>(&raw).is_ok());
        for bad in [
            raw.replacen("{", "{\"version\":1,", 1),
            raw.replacen("{", "{\"seed\":0,", 1),
            raw.replacen("\"count\":1", "\"count\":1.0", 1),
        ] {
            assert!(crate::codec::exact::<AdmissionRequest>(&bad).is_err());
        }
    }
}
