//! Host-only capability material. Persist verifier hashes; plaintext is returned
//! once to the operator and is never a Debug value or a public manifest field.
use mtg_contracts::{canonical, sha256};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialHashes {
    pub match_id: String,
    pub seats: [Option<[u8; 32]>; 4],
    pub operator: [u8; 32],
}
#[derive(Serialize)]
pub struct IssuedCapabilities {
    pub seats: [String; 4],
    pub operator: String,
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_secret(secret: &str) -> bool {
    secret.len() == 64
        && secret
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn verifier(match_id: &str, purpose: &str, secret: &str) -> [u8; 32] {
    let hex = sha256(
        canonical(&("capability-v1", match_id, purpose, secret))
            .expect("fixed string tuple")
            .as_bytes(),
    );
    std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("SHA256 hex"))
}
fn secret() -> Result<String, String> {
    // OS randomness belongs in hosting, never the deterministic rules core.
    // https://docs.rs/getrandom/0.4.3/getrandom/fn.fill.html
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|_| "capability randomness unavailable")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
impl CredentialHashes {
    pub fn issue(match_id: &str) -> Result<(Self, IssuedCapabilities), String> {
        if !valid_id(match_id) {
            return Err("invalid match identifier".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut issued = IssuedCapabilities {
            seats: std::array::from_fn(|_| String::new()),
            operator: String::new(),
        };
        for slot in issued
            .seats
            .iter_mut()
            .chain(std::iter::once(&mut issued.operator))
        {
            for _ in 0..16 {
                let candidate = secret()?;
                if seen.insert(candidate.clone()) {
                    *slot = candidate;
                    break;
                }
            }
            if slot.is_empty() {
                return Err("capability uniqueness unavailable".into());
            }
        }
        let hashes = Self {
            match_id: match_id.into(),
            seats: std::array::from_fn(|i| Some(verifier(match_id, "seat", &issued.seats[i]))),
            operator: verifier(match_id, "operator", &issued.operator),
        };
        Ok((hashes, issued))
    }
    pub fn authenticate_seat(&self, bearer: &str) -> Option<u8> {
        if !valid_secret(bearer) {
            return None;
        }
        let candidate = verifier(&self.match_id, "seat", bearer);
        let mut found = None;
        for (seat, stored) in self.seats.iter().enumerate() {
            let matches = bool::from(candidate.ct_eq(&stored.unwrap_or([0; 32])));
            if matches && stored.is_some() {
                if found.is_some() {
                    return None;
                }
                found = Some(seat as u8);
            }
        }
        found
    }
    pub fn authenticate_operator(&self, bearer: &str) -> bool {
        valid_secret(bearer)
            && bool::from(verifier(&self.match_id, "operator", bearer).ct_eq(&self.operator))
    }
    pub fn revoke(&mut self, seat: u8) -> Result<(), String> {
        let slot = self
            .seats
            .get_mut(usize::from(seat))
            .ok_or("invalid seat")?;
        *slot = None;
        Ok(())
    }
    pub fn rotate(&self, seat: u8) -> Result<(Self, String), String> {
        self.rotate_with(seat, secret)
    }
    fn rotate_with(
        &self,
        seat: u8,
        mut draw: impl FnMut() -> Result<String, String>,
    ) -> Result<(Self, String), String> {
        if seat >= 4 {
            return Err("invalid seat".into());
        }
        for _ in 0..16 {
            let token = draw()?;
            if !valid_secret(&token) {
                return Err("invalid capability material".into());
            }
            let candidate = verifier(&self.match_id, "seat", &token);
            let occupied = self
                .seats
                .iter()
                .flatten()
                .any(|hash| bool::from(candidate.ct_eq(hash)))
                || self.authenticate_operator(&token);
            if occupied {
                continue;
            }
            let mut next = self.clone();
            next.seats[usize::from(seat)] = Some(candidate);
            return Ok((next, token));
        }
        Err("capability uniqueness unavailable".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotation_retries_active_seat_and_operator_collisions_without_mutating_original() {
        let (original, issued) = CredentialHashes::issue("a123").unwrap();
        let fresh = "7".repeat(64);
        let mut candidates = vec![
            issued.seats[1].clone(),
            issued.seats[2].clone(),
            issued.operator.clone(),
            fresh.clone(),
        ]
        .into_iter();
        let (next, token) = original
            .rotate_with(1, || Ok(candidates.next().unwrap()))
            .unwrap();
        assert_eq!(token, fresh);
        assert_eq!(next.authenticate_seat(&token), Some(1));
        assert!(next.authenticate_seat(&issued.seats[1]).is_none());
        assert_eq!(original.authenticate_seat(&issued.seats[1]), Some(1));
        assert!(
            original
                .rotate_with(1, || Ok(issued.seats[2].clone()))
                .is_err()
        );
        assert_eq!(original.authenticate_seat(&issued.seats[2]), Some(2));
    }
    #[test]
    fn random_capabilities_are_seat_match_and_operator_scoped() {
        let (mut hashes, secrets) = CredentialHashes::issue("a123").unwrap();
        for seat in 0..4 {
            assert_eq!(
                hashes.authenticate_seat(&secrets.seats[seat]),
                Some(seat as u8)
            );
            assert!(!hashes.authenticate_operator(&secrets.seats[seat]));
        }
        assert!(hashes.authenticate_operator(&secrets.operator));
        assert!(hashes.authenticate_seat(&secrets.operator).is_none());
        let (other, _) = CredentialHashes::issue("b456").unwrap();
        for token in &secrets.seats {
            assert!(other.authenticate_seat(token).is_none());
        }
        let serialized = serde_json::to_string(&hashes).unwrap();
        for token in secrets
            .seats
            .iter()
            .chain(std::iter::once(&secrets.operator))
        {
            assert!(!serialized.contains(token));
        }
        let (rotated, new) = hashes.rotate(1).unwrap();
        assert!(rotated.authenticate_seat(&secrets.seats[1]).is_none());
        assert_eq!(rotated.authenticate_seat(&new), Some(1));
        assert_eq!(hashes.authenticate_seat(&secrets.seats[1]), Some(1));
        hashes.revoke(2).unwrap();
        assert!(hashes.authenticate_seat(&secrets.seats[2]).is_none());
        assert!(hashes.rotate(4).is_err());
        for token in [
            "",
            "secret",
            &"A".repeat(64),
            &"a".repeat(63),
            &"a".repeat(65),
        ] {
            assert!(hashes.authenticate_seat(token).is_none());
            assert!(!hashes.authenticate_operator(token));
        }
    }
}
