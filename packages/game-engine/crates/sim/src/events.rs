//! Trusted host instrumentation. Inputs are never exposed to a player policy.
pub use crate::analytics::PodOutcome;
use mtg_core::{Answer, Game, Observation, TraceRecord};

pub trait EventSink {
    fn record(&mut self, record: &TraceRecord) -> Result<(), String>;
    fn decision(&mut self, observation: &Observation, answer: &Answer) -> Result<(), String>;
    fn finish(&mut self, outcome: &PodOutcome) -> Result<(), String>;
}

pub(crate) fn drain(game: &mut Game, sink: &mut dyn EventSink) -> Result<(), String> {
    let mut result = Ok(());
    game.drain_trace(&mut |record| {
        if result.is_ok() {
            result = sink.record(record);
        }
    });
    result
}
