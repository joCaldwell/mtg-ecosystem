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
    include_str!("../../../../tests/fixtures/commander-policy.json");
pub fn policy_digest() -> String {
    sha256(COMMANDER_POLICY_JSON.as_bytes())
}
pub fn banned_commander_card(oracle_id: &str) -> Option<&'static str> {
    NAMED_BANS
        .iter()
        .chain(OFFENSIVE_BANS.iter())
        .find(|(id, _)| *id == oracle_id)
        .map(|(_, name)| *name)
}
const NAMED_BANS: [(&str, &str); 42] = [
    ("550c74d4-1fcb-406a-b02a-639a760a4380", "Ancestral Recall"),
    ("17fa98cd-ed8f-483f-9525-7e989a82ebb2", "Balance"),
    ("5089ec1a-f881-4d55-af14-5d996171203b", "Black Lotus"),
    ("d1b815d1-2848-40d4-a555-66822d1becbc", "Channel"),
    ("edb455f4-8dc9-4b7c-b25c-cb51b7dfbb41", "Chaos Orb"),
    (
        "697bcfe1-ecbf-42a1-bfc7-0766d48ca56b",
        "Dockside Extortionist",
    ),
    (
        "900ca697-ad38-4b2b-bc74-2ff7eb6ea951",
        "Emrakul, the Aeons Torn",
    ),
    (
        "d83bb32d-f409-421a-9d93-bd7ea3240b47",
        "Erayo, Soratami Ascendant",
    ),
    ("f5ca7b13-8003-4361-b827-7095c89f2750", "Falling Star"),
    ("e27193b7-1a47-4555-865d-b1fd4c6d597f", "Fastbond"),
    ("3e057471-e415-4105-9064-8236a0a5047d", "Flash"),
    (
        "efe9c3e0-7315-408e-9903-9d25a681039b",
        "Golos, Tireless Pilgrim",
    ),
    ("f759d112-76db-4091-a22b-b9f19ab6fa5f", "Griselbrand"),
    ("2f533667-e29b-4bec-897a-e7a9eee08314", "Hullbreacher"),
    (
        "5e816472-561d-43d1-ad28-80a829e2a91a",
        "Iona, Shield of Emeria",
    ),
    ("23216e59-f147-4bb7-a698-12777c2f8584", "Jeweled Lotus"),
    ("59119143-c0fa-49dd-adf0-e2fd3029c48b", "Karakas"),
    (
        "d5d91377-fd66-4dbe-a092-07f2ea379ca7",
        "Leovold, Emissary of Trest",
    ),
    (
        "2111588d-9af5-4a33-989e-b074d83f0463",
        "Library of Alexandria",
    ),
    ("5b287dcc-f853-4df1-8ddf-5b8b2adc2873", "Limited Resources"),
    ("2c63e4e1-89d2-4bc6-a232-94e75c4b1c8a", "Mana Crypt"),
    ("376ee366-e082-402f-b4db-6592fcfcacd2", "Mox Emerald"),
    ("0677f49e-f8bf-4349-af52-2ccde9287c2e", "Mox Jet"),
    ("824597b8-c89a-47ec-8526-7efc6e24ef0e", "Mox Pearl"),
    ("ed85fa82-e4fa-434b-92a8-36b6075708d1", "Mox Ruby"),
    ("d5ed1233-df87-4b90-8918-13922ec95249", "Mox Sapphire"),
    (
        "0ff11a61-6fe3-4c70-aba8-417023a5e54e",
        "Nadu, Winged Wisdom",
    ),
    ("d38937a6-ab1c-4de8-a278-d81a6ce0d921", "Paradox Engine"),
    ("ae83ef2c-960f-4c5b-97cc-52465c687c18", "Primeval Titan"),
    ("372351fd-58f7-4cb8-8644-5fbc94ccb145", "Prophet of Kruphix"),
    (
        "a6708b11-1bcd-4208-a967-fe91f2e3313c",
        "Recurring Nightmare",
    ),
    (
        "3015882b-4897-4d2c-8e33-6731c85a0d03",
        "Rofellos, Llanowar Emissary",
    ),
    ("fc04f621-338b-4d4c-bf57-5e5a00d990f0", "Shahrazad"),
    ("d17d071d-dbee-4d65-8435-137f35cd89ca", "Sundering Titan"),
    ("52b88034-bf0f-4d29-b776-40ce421f1107", "Sylvan Primordial"),
    ("99d4d99d-cf56-45aa-aa39-a250695612f2", "Time Vault"),
    ("d0209d3f-3f7e-4fd5-bce5-10bce6f29c86", "Time Walk"),
    ("254878f0-be90-4653-a395-0c41258fceaf", "Tinker"),
    ("dba4fd31-8931-42dd-bd86-45479c2abf74", "Tolarian Academy"),
    ("50885640-cdf1-4c62-b3bc-f37db6ab38b5", "Trade Secrets"),
    ("7cafc972-a6f5-4cac-a3d3-8a3ae36ffb1e", "Upheaval"),
    ("f7f76f39-a0de-4bda-86b6-0f291892fcec", "Yawgmoth's Bargain"),
];
const OFFENSIVE_BANS: [(&str, &str); 7] = [
    ("a610c77c-fe31-4465-a1c1-392db4ce4ed1", "Cleanse"),
    ("4692740f-be90-459f-8d90-c4ae71771595", "Crusade"),
    ("632de66b-2314-4299-847c-16a84bf9121f", "Imprison"),
    ("854ad486-0c59-4c57-9a76-ab1dff0ff37c", "Invoke Prejudice"),
    ("b18b9869-8490-4875-a5bb-484c3299f2c5", "Jihad"),
    ("37c49483-bef6-47c6-9354-ead8560d48da", "Pradesh Gypsies"),
    (
        "124c8663-21f3-4cd8-a060-9d04be35c43f",
        "Stone-Throwing Devils",
    ),
];
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
        for ((id, name), item) in NAMED_BANS.iter().zip(bans) {
            assert_eq!(item["oracle_id"], *id);
            assert_eq!(item["name"], *name);
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
            assert_eq!(record["oracleIdentity"], *id);
            assert_eq!(record["sourceSha256"], item["source_sha256"]);
            assert_eq!(record["sourceIndex"], item["source_index"]);
            let source_name = record["source"]["name"].as_str().unwrap();
            assert!(
                source_name == *name
                    || record["source"]["card_faces"]
                        .as_array()
                        .is_some_and(|faces| faces.iter().any(|f| f["name"] == *name))
            );
            assert_eq!(banned_commander_card(id), Some(*name));
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
        for ((id, name), item) in OFFENSIVE_BANS.iter().zip(records) {
            assert_eq!(item["oracle_id"], *id);
            assert_eq!(item["name"], *name);
            assert_eq!(banned_commander_card(id), Some(*name));
            let raw = item["record_json"].as_str().unwrap();
            assert_eq!(sha256(raw.as_bytes()), item["record_sha256"]);
            let record: serde_json::Value = serde_json::from_str(raw).unwrap();
            assert_eq!(record["oracleIdentity"], *id);
            assert_eq!(record["source"]["name"], *name);
        }
        let pool = p["admitted_pool_review"].as_array().unwrap();
        assert_eq!(pool.len(), 34);
        for card in pool {
            assert_eq!(card["conspiracy_type"], false);
            assert_eq!(card["ante_reference"], false);
            assert_eq!(card["offensive_named_ban"], false);
            assert!(banned_commander_card(card["oracle_id"].as_str().unwrap()).is_none());
        }
    }
}
