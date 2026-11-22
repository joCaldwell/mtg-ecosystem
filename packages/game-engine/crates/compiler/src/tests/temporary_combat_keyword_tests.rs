//! Exact d705 archive witnesses; mutations below are synthetic compiler probes.
//! CR 205.3k, 601.2c, 608.2c, 611.2a/c, 613.1f/4c, 514.2,
//! 702.7, 702.10 and 702.19 (pinned 2026-09-25): one captured target,
//! ordered stat/keyword effects and end-of-turn duration use the existing family.
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
//! Native tests are UNRUN by this child; parent owns combined qualification.
use super::*;
use serde_json::{Value, json};
#[test]
fn temporary_combat_keyword_strict_gained_decoder() {
    let valid = json!([
        {"gained":"keyword","keyword":{"keyword":"haste"}},
        {"gained":"keyword","keyword":{"keyword":"first strike"}},
        {"gained":"keyword","keyword":{"keyword":"trample"}}
    ]);
    assert_eq!(
        lower_temporary_creature_keywords(valid.as_array().unwrap()).unwrap(),
        [
            KeywordAbility::Haste,
            KeywordAbility::FirstStrike,
            KeywordAbility::Trample
        ]
    );
    let malformed = [
        Value::Null,
        json!(false),
        json!(2),
        json!("haste"),
        json!([]),
        json!({}),
        json!({"gained":"keyword"}),
        json!({"gained":"keyword","keyword":null}),
        json!({"gained":"keyword","keyword":{"keyword":null}}),
        json!({"gained":"keyword","keyword":{"keyword":"first-strike"}}),
        json!({"gained":"keyword","keyword":{"keyword":"defender"}}),
        json!({"gained":"activated","keyword":{"keyword":"haste"}}),
        json!({"gained":"triggered","keyword":{"keyword":"haste"}}),
        json!({"gained":"keyword","keyword":{"keyword":"haste"},"extra":null}),
        json!({"gained":"keyword","keyword":{"keyword":"haste","cost":[]}}),
        json!({"gained":"keyword","keyword":{"keyword":"haste","filter":null}}),
    ];
    for bad in malformed {
        for pair in [
            [valid[0].clone(), bad.clone()],
            [bad.clone(), valid[0].clone()],
        ] {
            assert!(lower_temporary_creature_keywords(&pair).is_err(), "{bad}");
        }
    }
    assert!(lower_temporary_creature_keywords(&[]).is_err());
    for keyword in valid.as_array().unwrap() {
        assert!(lower_temporary_creature_keywords(&[keyword.clone(), keyword.clone()]).is_err());
    }
}
