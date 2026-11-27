//! Rules time is independent of event IDs and trace (CR 613.7a–g, m–n).
//! https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TimestampAllocator(u64);

impl Default for TimestampAllocator {
    fn default() -> Self {
        Self(1)
    }
}

impl TimestampAllocator {
    #[cfg(feature = "testkit")]
    pub fn test_next(self) -> u64 {
        self.next()
    }

    #[cfg(feature = "testkit")]
    pub fn test_allocate(&mut self) -> u64 {
        self.allocate().unwrap()
    }

    pub(super) fn next(self) -> u64 {
        self.0
    }

    pub(super) fn contains(self, timestamp: u64) -> bool {
        timestamp != 0 && timestamp < self.0
    }

    pub(super) fn allocate(&mut self) -> Result<u64, String> {
        let timestamp = self.next();
        if timestamp == 0 {
            return Err("invalid timestamp allocator".into());
        }
        self.0 = timestamp.checked_add(1).ok_or("timestamp resource limit")?;
        Ok(timestamp)
    }

    pub(super) fn is_initial(&self) -> bool {
        *self == Self::default()
    }
}

impl RulesTxn {
    /// An ordered occurrence gets its own timestamp. Members of the same
    /// simultaneous occurrence share a time, with object ties further ordered
    /// by the APNAP rank recorded at that time (CR 613.7m).
    pub(super) fn allocate_timestamp(&mut self, common: Option<&Self>) -> Result<u64, String> {
        if let Some(common) = common {
            let timestamp = common.state.rules.timestamps.next();
            if self.state.rules.timestamps.contains(timestamp) {
                return Ok(timestamp);
            }
        }
        self.rules_mut().timestamps.allocate()
    }
}
