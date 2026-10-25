//! Commander policy is selected by each profile's dated data reference.
use super::match_profile;
pub fn policy_digest(profile: &str) -> Result<String, String> {
    match_profile(profile)
        .map(|p| p.policy_digest())
        .ok_or_else(|| "unknown match profile".into())
}
#[cfg(test)]
mod tests {
    use crate::engine::match_admission::BANNED_LISTS;
    use mtg_contracts::sha256;
    #[test]
    fn all_named_ban_ids_are_bound_to_complete_authentic_source_records() {
        for raw in BANNED_LISTS {
            let p: serde_json::Value = mtg_contracts::strict_read(raw).unwrap();
            assert_eq!(p["html_sha256"].as_str().unwrap().len(), 64);
            assert!(p["url"].as_str().unwrap().starts_with("https://"));
            let bans = p["named_bans"].as_array().unwrap();
            assert!(!bans.is_empty());
            for item in bans {
                let id = item["oracle_id"].as_str().unwrap();
                let name = item["name"].as_str().unwrap();
                assert_eq!(item["oracle_id"], id);
                assert_eq!(item["name"], name);
                let raw = item["record_json"].as_str().unwrap();
                assert_eq!(sha256(raw.as_bytes()), item["record_sha256"]);
                let record: serde_json::Value = mtg_contracts::strict_read(raw).unwrap();
                #[derive(serde::Deserialize)]
                struct RawSource {
                    source: Box<serde_json::value::RawValue>,
                }
                let source: RawSource = serde_json::from_str(raw).unwrap();
                assert_eq!(
                    sha256(source.source.get().as_bytes()),
                    item["source_sha256"]
                );
                assert_eq!(record["oracleIdentity"], id);
                assert_eq!(record["sourceSha256"], item["source_sha256"]);
                assert_eq!(record["sourceIndex"], item["source_index"]);
                let source_name = record["source"]["name"].as_str().unwrap();
                assert!(
                    source_name == name
                        || record["source"]["card_faces"]
                            .as_array()
                            .is_some_and(|faces| faces.iter().any(|f| f["name"] == name))
                );
            }

            assert_eq!(p["categories"]["lutri"], "companion_only");
        }
    }
    #[test]
    fn offensive_category_and_finite_pool_exclusions_are_source_bound() {
        for raw in BANNED_LISTS {
            let p: serde_json::Value = mtg_contracts::strict_read(raw).unwrap();
            assert_eq!(
                p["offensive_policy"]["html_sha256"].as_str().unwrap().len(),
                64
            );
            let records = p["offensive_policy"]["records"].as_array().unwrap();
            assert!(!records.is_empty());
            for item in records {
                let id = item["oracle_id"].as_str().unwrap();
                let name = item["name"].as_str().unwrap();
                assert_eq!(item["oracle_id"], id);
                assert_eq!(item["name"], name);

                let raw = item["record_json"].as_str().unwrap();
                assert_eq!(sha256(raw.as_bytes()), item["record_sha256"]);
                let record: serde_json::Value = serde_json::from_str(raw).unwrap();
                assert_eq!(record["oracleIdentity"], id);
                assert_eq!(record["source"]["name"], name);
            }
        }
    }
}
