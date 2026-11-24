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
#[allow(unused_imports)]
pub(crate) use engine::*;
pub use engine::{
    ACTION_PAGE_MAX, AcceptedInput, AcceptedInputCounts, ActionBeginning, ActionContinuation,
    ActionDecisionContext, ActionInitiation, ActionLabel, ActionPage, ActionQueryOutcome,
    ActionQueryResult, ActionScope, ActionSourceContext, ActionUnavailable, ActualDamage,
    AdditionalCostChoice, Advance, AdvanceReport, AttachmentBefore, BatchCause, Budget,
    CaptureOutcome, CaptureWork, CapturedSearchContext, CardGapCode, CardGapIssue, CardGaps,
    CardId, CardSetup, CastOccurrence, CastOccurrenceId, ColorIdentityPolicy,
    CommanderDamageBefore, CommanderDamageResult, CommanderEligibility, CommanderHistory,
    CommanderLedger, CommanderPairing, CommanderPolicy, CommittedBatch, CompletionLayout,
    CoreInputIdentity, CounterDivision, CounterPlacementEffectHandle, CounterPlacementEffectKey,
    CreatureLossReason, DYNAMIC_MAX_DECK_ROWS, DYNAMIC_PROFILE, DamageApplication,
    DamageEffectCandidate, DamageEffectKind, DamageHandle, DamageId, DamageKind, DamageOccurrence,
    DamageRecipient, DamageReplacementQualification, DamageResult, DamageTransactionBefore,
    DamageTransactionId, DecisionContext, DecisionInstruction, DecisionMode, DecisionSource,
    DecisionSourceKind, DecisionToken, DeckEntry, DeckGapCode, DeckInspection, DeckRole, Decklist,
    DynamicAdmissionError, DynamicAdmissionIdentity, DynamicDeckEntry, DynamicMatchConfig,
    DynamicPod, DynamicPodTemplate, EffectHandle, EffectId, EffectKey, EffectOrigin,
    EffectiveTriggerAbility, EntryCast, Event, EventGroup, EventOutcome, EvidenceRecord,
    ExternalInput, Feature, Game, GraveyardOrderOption, History, HypotheticalCard,
    InputProcessingCause, InputProcessingFact, InterveningCheck, InterveningStage, KnownHandMemory,
    KnownLibraryPosition, LegalBlockers, LibraryBefore, LifelinkGain, MATCH_PROFILES,
    ManaAbilityIdentity, MatchAdmissionError, MatchConfig, MatchProfile, MatchRandom, MatchState,
    Mode, MoveOutcome, NoncombatDamageOccurrence, ObjectBefore, ObjectHandle, ObjectId,
    Observation, OccurrenceId, OccurrenceResult, OpenDecklist, OpeningPlayer, OpeningSetup,
    OwnKnowledgeAction, PermanentManaUndo, PermittedHistory, PermittedView, Phase, PhaseReturn,
    PhasingSchedule, PlayerBefore, PlayerId, PlayerLossReason, PlayerRequest, PlayerResponse,
    PlayerSession, PlayerStatus, PlayerTargetEntity, PreEventFacts, Prepared, PreparedDamage,
    ProfileError, ProposedEvent, PublicSearchProfile, QualifyingCreature, QueryRejection,
    QueryResult, QueryValue, RememberedCard, ReplayJournal, Request, ResolvedEffect,
    ResolvedEffectKind, Response, RngPosition, RootHandleMap, RuleFault, SEARCH_INFORMATION,
    SEARCH_RECONSTRUCTION, SEARCH_SAMPLER, SampleBuildOutcome, SampleBuildWork,
    SampleRestoreOutcome, SampleRestoreWork, SampledGame, ScenarioConfig, ScenarioInitial,
    SearchCapture, SearchCaptureData, SearchContext, SearchRejection, SeatHypothesis, SetupAction,
    SetupOutcome, SetupPhase, Snapshot, SnapshotVerifier, SnapshotWriteInfo, SourceInformation,
    StackBefore, StackHandle, StackId, TOKEN_CAPACITY, TargetEntity, TraceEvidence, TraceMode,
    TraceRecord, TraceVerification, TriggerEventWitness, TriggerHandle, TriggerId,
    TriggerOccurrenceId, TriggerPlacementFailure, TurnBefore, TurnState, TurnStep, ValidatedPod,
    ValidatedResponse, VisibleAbility, VisibleAttacker, VisibleCommander,
    VisibleCommanderReturnChoice, VisibleCounterPlacementCandidate, VisibleDamageEffectCandidate,
    VisibleEffect, VisibleEffectSource, VisibleEntityTarget, VisibleObject, VisiblePendingTrigger,
    VisiblePermanentCopy, VisiblePlayer, VisibleSetup, VisibleSetupPhase, VisibleSetupPlayer,
    VisibleStackEntry, VisibleStackTarget, VisibleTarget, VisibleToken, VisibleTriggerAbility,
    VisibleTriggered, Work, WorldProposal, Zone, admit_dynamic_pod, inspect_dynamic_deck,
    match_profile, policy_digest, validate_pod,
};
