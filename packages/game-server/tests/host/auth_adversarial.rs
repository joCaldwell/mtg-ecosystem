//! Independent capability-verifier tests, not HTTP/registry authentication proof.
use mtg_contracts::{canonical, strict_read};
use mtg_server::auth::CredentialHashes;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn restored(hashes: &CredentialHashes) -> CredentialHashes {
    strict_read(&canonical(hashes).unwrap()).unwrap()
}

#[test]
fn duplicate_private_seat_hashes_fail_closed_in_every_row_order() {
    let (original, issued) = CredentialHashes::issue("0123456789abcdef").unwrap();
    for from in 0..4 {
        for to in 0..4 {
            if from == to {
                continue;
            }
            let mut forged = original.clone();
            forged.seats[to] = original.seats[from];
            let forged = restored(&forged);
            assert_eq!(forged.authenticate_seat(&issued.seats[from]), None);
            assert_eq!(forged.authenticate_seat(&issued.seats[to]), None);
            for unaffected in (0..4).filter(|seat| *seat != from && *seat != to) {
                assert_eq!(
                    forged.authenticate_seat(&issued.seats[unaffected]),
                    Some(unaffected as u8)
                );
            }
            assert!(forged.authenticate_operator(&issued.operator));
        }
    }
}

#[test]
fn persisted_role_hash_substitution_and_match_relabeling_cannot_promote_tokens() {
    let (original, issued) = CredentialHashes::issue("a123").unwrap();
    for seat in 0..4 {
        let mut forged = original.clone();
        forged.operator = original.seats[seat].unwrap();
        forged.seats[seat] = Some(original.operator);
        let forged = restored(&forged);
        assert!(!forged.authenticate_operator(&issued.seats[seat]));
        assert!(!forged.authenticate_operator(&issued.operator));
        assert_eq!(forged.authenticate_seat(&issued.operator), None);
        assert_eq!(forged.authenticate_seat(&issued.seats[seat]), None);
    }
    let mut relabeled = original;
    relabeled.match_id = "b456".into();
    let relabeled = restored(&relabeled);
    for token in &issued.seats {
        assert_eq!(relabeled.authenticate_seat(token), None);
    }
    assert!(!relabeled.authenticate_operator(&issued.operator));
}

#[test]
fn persisted_revocation_and_rotation_preserve_only_the_intended_authority() {
    let (mut hashes, issued) = CredentialHashes::issue("abcdef").unwrap();
    hashes.revoke(0).unwrap();
    hashes.revoke(3).unwrap();
    let (rotated, new_token) = restored(&hashes).rotate(0).unwrap();
    assert_eq!(hashes.authenticate_seat(&new_token), None);
    let rotated = restored(&rotated);
    assert_eq!(rotated.authenticate_seat(&issued.seats[0]), None);
    assert_eq!(rotated.authenticate_seat(&new_token), Some(0));
    assert_eq!(rotated.authenticate_seat(&issued.seats[3]), None);
    for seat in [1, 2] {
        assert_eq!(
            rotated.authenticate_seat(&issued.seats[seat]),
            Some(seat as u8)
        );
    }
    assert!(rotated.authenticate_operator(&issued.operator));
    let before = canonical(&hashes).unwrap();
    assert!(hashes.revoke(255).is_err());
    assert!(hashes.rotate(255).is_err());
    assert_eq!(before, canonical(&hashes).unwrap());
}

#[test]
fn private_hash_record_shape_is_strict_and_persistence_has_no_plaintext() {
    let (hashes, issued) = CredentialHashes::issue("a").unwrap();
    let serialized = canonical(&hashes).unwrap();
    let value: Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 3);
    for token in issued.seats.iter().chain(std::iter::once(&issued.operator)) {
        assert_eq!(token.len(), 64);
        assert!(
            token
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert!(!serialized.contains(token));
    }
    assert_eq!(
        issued
            .seats
            .iter()
            .chain(std::iter::once(&issued.operator))
            .collect::<BTreeSet<_>>()
            .len(),
        5
    );
    let mut negatives = vec![];
    let mut extra = value.clone();
    extra["secret"] = json!(issued.seats[0]);
    negatives.push(extra);
    let mut missing = value.clone();
    missing.as_object_mut().unwrap().remove("operator");
    negatives.push(missing);
    let mut short_hash = value.clone();
    short_hash["seats"][0] = json!(vec![0; 31]);
    negatives.push(short_hash);
    let mut invalid_byte = value.clone();
    invalid_byte["operator"][0] = json!(256);
    negatives.push(invalid_byte);
    let mut wrong_rows = value.clone();
    wrong_rows["seats"].as_array_mut().unwrap().pop();
    negatives.push(wrong_rows);
    let mut raw_token = value;
    raw_token["seats"][0] = json!(issued.seats[0]);
    negatives.push(raw_token);
    for negative in negatives {
        assert!(strict_read::<CredentialHashes>(&negative.to_string()).is_err());
    }
    let duplicate = serialized.replacen('{', "{\"match_id\":\"b\",", 1);
    assert!(strict_read::<CredentialHashes>(&duplicate).is_err());
}

#[test]
fn issuance_and_bearer_spelling_enforce_the_declared_finite_boundary() {
    for invalid in ["", "A", "../a", "a/b", "a\\b", "a:b", "a?b", "é", "a\0b"] {
        assert!(CredentialHashes::issue(invalid).is_err());
    }
    assert!(CredentialHashes::issue(&"a".repeat(65)).is_err());
    let (hashes, issued) = CredentialHashes::issue(&"a".repeat(64)).unwrap();
    for invalid in [
        format!("Bearer {}", issued.seats[0]),
        format!("{}\n", issued.seats[0]),
        format!(" {}", issued.operator),
        "0".repeat(63),
        "0".repeat(65),
        "A".repeat(64),
        "é".repeat(32),
    ] {
        assert_eq!(hashes.authenticate_seat(&invalid), None);
        assert!(!hashes.authenticate_operator(&invalid));
    }
    // These tests exercise issuance validation. A registry loading a private
    // CredentialHashes DTO must separately validate/bind its configured match ID.
}
