//! Exact private DTO decoding. Serde's internally tagged unit variants can
//! silently drop unknown fields even with deny_unknown_fields. Re-encoding the
//! typed value must preserve the duplicate-checked input Value completely.
use serde::{Serialize, de::DeserializeOwned};
pub fn exact<T: Serialize + DeserializeOwned>(text: &str) -> Result<T, String> {
    let value: serde_json::Value = mtg_contracts::strict_read(text)?;
    let decoded: T = serde_json::from_value(value.clone()).map_err(|_| "invalid private DTO")?;
    if serde_json::to_value(&decoded).map_err(|_| "invalid private DTO")? != value {
        return Err("private DTO fields are not exact".into());
    }
    Ok(decoded)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::CommandExecutionStatus;
    #[test]
    fn tagged_unit_rows_reject_fields_that_serde_otherwise_drops() {
        assert!(exact::<CommandExecutionStatus>(r#"{"status":"pending"}"#).is_ok());
        assert!(exact::<CommandExecutionStatus>(r#"{"status":"pending","ignored":true}"#).is_err());
        assert!(
            exact::<CommandExecutionStatus>(r#"{"status":"pending","status":"pending"}"#).is_err()
        );
    }
}
