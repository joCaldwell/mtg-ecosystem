//! Immutable current official Commander named-ban policy. The selected pool
//! separately rejects unsupported names/mechanics; this is not a general
//! legality service. Official source and raw archive facts are retained in the
//! reviewed fixture. Lutri is companion-only, not a universal named card ban.
use mtg_contracts::sha256;
pub const COMMANDER_POLICY_URL: &str = "https://magic.wizards.com/en/banned-restricted-list";
pub const COMMANDER_POLICY_DATE: &str = "2026-10-15";
pub const COMMANDER_POLICY_HTML_SHA256: &str =
    "def38a80a72f7da25d7a0c13ba82bfa2e0b4139f2954f9cf02d1d7cbf134d938";
pub const COMMANDER_POLICY_JSON: &str =
    include_str!("../../../../tests/fixtures/commander-banned-2026-10-15.json");
pub fn policy_digest() -> String {
    sha256(COMMANDER_POLICY_JSON.as_bytes())
}
static BANNED: std::sync::LazyLock<serde_json::Value> = std::sync::LazyLock::new(|| {
    mtg_contracts::strict_read(COMMANDER_POLICY_JSON).expect("dated banned list")
});
pub fn banned_commander_card(oracle_id: &str) -> Option<&'static str> {
    BANNED["named_bans"]
        .as_array()
        .unwrap()
        .iter()
        .chain(BANNED["offensive_policy"]["records"].as_array().unwrap())
        .find(|b| b["oracle_id"].as_str() == Some(oracle_id))
        .and_then(|b| b["name"].as_str())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_named_ban_ids_are_bound_to_complete_authentic_source_records() {
        let p: serde_json::Value = mtg_contracts::strict_read(COMMANDER_POLICY_JSON).unwrap();
        assert_eq!(p["html_sha256"], COMMANDER_POLICY_HTML_SHA256);
        assert_eq!(p["url"], COMMANDER_POLICY_URL);
        let bans = p["named_bans"].as_array().unwrap();
        assert_eq!(bans.len(), 42);
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
            assert_eq!(banned_commander_card(id), Some(name));
        }
        assert_eq!(
            banned_commander_card("cfb756dd-6fe1-430a-8f04-8b10cac62a86"),
            None
        );
        assert_eq!(p["categories"]["lutri"], "companion_only");
    }
    #[test]
    fn offensive_category_and_finite_pool_exclusions_are_source_bound() {
        let p: serde_json::Value = mtg_contracts::strict_read(COMMANDER_POLICY_JSON).unwrap();
        assert_eq!(
            p["offensive_policy"]["html_sha256"],
            "91f536b644fc278d62c840865a499bc883c2fd086831ddddcdc83adf316322fa"
        );
        let records = p["offensive_policy"]["records"].as_array().unwrap();
        assert_eq!(records.len(), 7);
        for item in records {
            let id = item["oracle_id"].as_str().unwrap();
            let name = item["name"].as_str().unwrap();
            assert_eq!(item["oracle_id"], id);
            assert_eq!(item["name"], name);
            assert_eq!(banned_commander_card(id), Some(name));
            let raw = item["record_json"].as_str().unwrap();
            assert_eq!(sha256(raw.as_bytes()), item["record_sha256"]);
            let record: serde_json::Value = serde_json::from_str(raw).unwrap();
            assert_eq!(record["oracleIdentity"], id);
            assert_eq!(record["source"]["name"], name);
        }
    }
}
