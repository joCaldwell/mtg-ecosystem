use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceMode {
    Off,
    Compact,
    Detailed,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceVerification {
    pub records: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvidenceRecord {
    Event { event: Box<Event> },
    Batch { batch: Box<CommittedBatch> },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceRecord {
    pub sequence: u64,
    pub boundary: u64,
    pub decisions_seen: u64,
    pub concessions_seen: u64,
    pub record: EvidenceRecord,
}
/// Trusted native evidence. Never send to a player or use for rules evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceEvidence {
    pub mode: TraceMode,
    pub verification: Option<TraceVerification>,
    pub records: Vec<TraceRecord>,
    pub events: Vec<Event>,
}
impl TraceEvidence {
    fn new(mode: TraceMode) -> Self {
        Self {
            mode,
            verification: (mode != TraceMode::Off).then(|| TraceVerification {
                records: 0,
                sha256: sha256(b"mtg-event-trace-v1"),
            }),
            records: vec![],
            events: vec![],
        }
    }
}
impl Game {
    pub fn with_trace(prepared: Prepared, mode: TraceMode) -> Self {
        let mut game = Self::new(prepared);
        game.trace = TraceEvidence::new(mode);
        game
    }
    pub fn trace(&self) -> &TraceEvidence {
        &self.trace
    }
    pub(super) fn initial_trace() -> TraceEvidence {
        TraceEvidence::new(TraceMode::Detailed)
    }
    pub(super) fn record_event(&mut self, event: Event) -> Result<(), String> {
        self.record_evidence(EvidenceRecord::Event {
            event: Box::new(event),
        })
    }
    pub(super) fn record_evidence(&mut self, record: EvidenceRecord) -> Result<(), String> {
        let sequence = self.state.next_event_sequence;
        self.state.next_event_sequence = sequence
            .checked_add(1)
            .ok_or("event sequence resource limit")?;
        if self.trace.mode == TraceMode::Off {
            return Ok(());
        }
        let entry = TraceRecord {
            sequence,
            boundary: self.state.rule_steps,
            decisions_seen: u64::try_from(self.state.journal.len())
                .map_err(|_| "trace input count resource limit")?,
            concessions_seen: u64::try_from(self.state.external_inputs.len())
                .map_err(|_| "trace input count resource limit")?,
            record,
        };
        let verification = self
            .trace
            .verification
            .as_mut()
            .ok_or("trace verification invariant")?;
        let mut bytes = verification.sha256.as_bytes().to_vec();
        bytes.push(0);
        bytes.extend_from_slice(canonical(&entry)?.as_bytes());
        verification.sha256 = sha256(&bytes);
        verification.records = self.state.next_event_sequence;
        if self.trace.mode == TraceMode::Detailed {
            if let EvidenceRecord::Event { event } = &entry.record {
                self.trace.events.push((**event).clone());
            }
            self.trace.records.push(entry);
        }
        Ok(())
    }
    pub fn replay_with_trace(
        prepared: Prepared,
        journal: &ReplayJournal,
        mode: TraceMode,
    ) -> Result<Self, String> {
        Self::replay_to(prepared, journal, None, mode)
    }
}
