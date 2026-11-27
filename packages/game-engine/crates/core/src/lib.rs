//! Deterministic scenario kernel. See frozen first-scenario brief for admission.

/// A public struct whose fields keep their declared (private) visibility in
/// production and become `pub` only under the `testkit` feature, so external
/// white-box test binaries can build and inspect states that only the engine
/// constructs. Production code never sees the difference.
macro_rules! testkit_fields {
    ($(#[$meta:meta])* pub struct $name:ident $(<$($param:ident),+>)? {
        $($(#[$field_meta:meta])* $vis:vis $field:ident: $ty:ty),* $(,)?
    }) => {
        #[cfg(not(feature = "testkit"))]
        $(#[$meta])* pub struct $name $(<$($param),+>)? {
            $($(#[$field_meta])* $vis $field: $ty),*
        }
        #[cfg(feature = "testkit")]
        $(#[$meta])* pub struct $name $(<$($param),+>)? {
            $($(#[$field_meta])* pub $field: $ty),*
        }
    };
}

mod build_identity;
pub use build_identity::{BuildIdentity, build_id, build_identity, install_build_identity};
mod engine;
pub mod formats;
mod profile_data;
pub mod query;
#[cfg(feature = "testkit")]
pub mod testkit;
// The public API is this explicit list. Engine internals are `pub` only where
// external test binaries need them; they stay unreachable unless `testkit`
// re-exports them (see testkit.rs).
pub use engine::ControllerMeasurement;
#[allow(unused_imports)]
pub(crate) use engine::*;
pub use engine::{
    AcceptedInput, AcceptedInputCounts, Action, ActualDamage, Advance, AdvanceReport, Answer,
    AttachmentBefore, BatchCause, Bucket, Budget, Candidate, CandidateLabel, CaptureOutcome,
    CaptureWork, CapturedSearchContext, CardGapCode, CardGapIssue, CardGaps, CardId, CardSetup,
    CastOccurrence, CastOccurrenceId, ColorIdentityPolicy, CommanderDamageBefore,
    CommanderDamageResult, CommanderEligibility, CommanderHistory, CommanderLedger,
    CommanderPairing, CommanderPolicy, CommittedBatch, CompletionLayout, CoreInputIdentity,
    CreatureLossReason, DYNAMIC_MAX_DECK_ROWS, DYNAMIC_PROFILE, DamageApplication,
    DamageEffectCandidate, DamageEffectKind, DamageHandle, DamageId, DamageKind, DamageOccurrence,
    DamageRecipient, DamageReplacementQualification, DamageResult, DamageTransactionBefore,
    DamageTransactionId, Decision, DecisionContext, DecisionInstruction, DecisionSource,
    DecisionSourceKind, DecisionToken, DeckEntry, DeckGapCode, DeckInspection, DeckRole, Decklist,
    DynamicAdmissionError, DynamicAdmissionIdentity, DynamicDeckEntry, DynamicMatchConfig,
    DynamicPod, DynamicPodTemplate, EffectHandle, EffectId, EffectKey, EffectOrigin,
    EffectiveTriggerAbility, EntryCast, Event, EventGroup, EventOutcome, EvidenceRecord,
    ExternalInput, Feature, Flag, Game, Gate, GraveyardOrderOption, History, HypotheticalCard,
    InputProcessingCause, InputProcessingFact, InterveningCheck, InterveningStage, JointRule,
    KnownHandMemory, KnownLibraryPosition, LibraryBefore, LifelinkGain, MATCH_PROFILES,
    ManaAbilityIdentity, ManaPool, MatchAdmissionError, MatchConfig, MatchProfile, MatchRandom,
    MatchState, Mode, MoveOutcome, NoncombatDamageOccurrence, ObjectBefore, ObjectHandle, ObjectId,
    Observation, OccurrenceId, OccurrenceResult, OpenDecklist, OpenStream, OpeningPlayer,
    OpeningSetup, OwnKnowledgeAction, PayAnswer, PayStep, PermanentManaUndo, PermittedHistory,
    PermittedView, Phase, PhaseReturn, PhasingSchedule, PlayerBefore, PlayerId, PlayerLossReason,
    PlayerSession, PlayerStatus, PlayerTargetEntity, PreEventFacts, Prepared, PreparedDamage,
    ProfileError, ProposedEvent, PublicSearchProfile, QualifyingCreature, QueryRejection,
    QueryResult, QueryValue, Reason, RememberedCard, ReplayJournal, ResolvedEffect,
    ResolvedEffectKind, RootHandleMap, RuleFault, Rules, SEARCH_INFORMATION, SEARCH_RECONSTRUCTION,
    SEARCH_SAMPLER, SampleBuildOutcome, SampleBuildWork, SampleRestoreOutcome, SampleRestoreWork,
    SampledGame, ScenarioConfig, ScenarioInitial, SearchCapture, SearchCaptureData, SearchContext,
    SearchRejection, SeatHypothesis, SetupAction, SetupOutcome, SetupPhase, Shape, Slot, SlotRole,
    Snapshot, SnapshotVerifier, SnapshotWriteInfo, SourceInformation, Space, StackBefore,
    StackHandle, StackId, TOKEN_CAPACITY, TargetEntity, TraceEvidence, TraceMode, TraceRecord,
    TraceVerification, TriggerEventWitness, TriggerHandle, TriggerId, TriggerOccurrenceId,
    TriggerPlacementFailure, TurnBefore, TurnHistoryUndo, TurnState, TurnStep, ValidatedPod,
    ValidatedResponse, Viewer, VisibleAbility, VisibleAttacker, VisibleCommander,
    VisibleCommanderReturnChoice, VisibleDamageEffectCandidate, VisibleEffect, VisibleEffectSource,
    VisibleEntityTarget, VisibleObject, VisiblePendingTrigger, VisiblePlayer, VisibleSetup,
    VisibleSetupPhase, VisibleSetupPlayer, VisibleStackEntry, VisibleStackTarget, VisibleTarget,
    VisibleToken, VisibleTriggerAbility, VisibleTriggered, Work, WorldProposal, Zone,
    admit_dynamic_pod, distinct_payment_choices, inspect_dynamic_deck, match_profile,
    payment_choice_minimum, policy_digest, validate_pod,
};
pub use mtg_view::combat_allocation_to_blockers;
