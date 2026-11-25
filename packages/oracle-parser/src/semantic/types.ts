import type { Amount, CascadeExiledCards, Condition, CharacteristicRelation, ChosenValueRef, Color, Cost, Effect, EntryCopy, ObjectFilter, ObjectRef, ParseCardResult, PlayerRef, ReplacementInstruction, StaticEffect, Trigger, ValueChoice, VariableConstraint } from "../ast.ts";
import type { KeywordAbilityKind } from "./keyword-scopes.ts";

/** Structural source input: callers may pass the complete Scryfall record.
 * Fields actually consumed by the analysis are also checked at runtime. */
export interface BindingSourceFace {
  name: string;
  type_line?: string | null;
  mana_cost?: string | null;
}
export interface BindingSourceCard extends BindingSourceFace {
  layout: string;
  card_faces?: BindingSourceFace[] | null;
}
export interface CardBindingInput {
  identity: string;
  source: BindingSourceCard;
  faces: { sourceFaceIndex: number; result?: ParseCardResult }[];
}

export type BindingPhase = "payment" | "resolution" | "continuous" | "trigger-match" | "replacement-match";
export type BindingCardinality = "one" | "many" | "unknown";
export type SymbolicAmount = Extract<Amount, { amount: "x" | "that-number" | "that-much" | "replacement-event-quantity" | "prevention-event-quantity" | "dice-result" }>;
/** `implicit` is a player relation printed inside a filter ("an opponent
 * controls", "you don't control"). */
export type ReferenceExpression = ObjectRef | CascadeExiledCards | { ref: "affected-object" } | { ref: "those-counters" } | { recipient: "they" } | PlayerRef | SymbolicAmount
  | ChosenValueRef | NonNullable<ObjectFilter["chosenType"]>
  | { implicit: "opponents" | "other-players" };

interface NodeBase { id: string; path: string; rules: string[] }
export type BindingNode = NodeBase & (
  /** CR 701.23b: the chosen cards in one hidden, quality-constrained search.
   * Each ordered slot accepts zero or one card; no object may fill two slots. */
  | { node: "distinct-library-search"; scope: string; chooser: string; libraryOwner: string;
      slots: Color[]; allocation: "distinct-objects"; perSlot: "zero-or-one"; cardinality: "many" }
  | { node: "source"; identity: string }
  | { node: "face"; source: string; sourceFaceIndex: number; name: string; typeLine?: string; manaCost?: string; layout: string }
  | { node: "ability"; source: string; face: string; abilityKind: string; you: string }
  /** One resolving cascade whose controller matches the static ability's "you". */
  | { node: "cascade-occurrence"; scope: string; controllerMatches: string }
  | { node: "cascade-exiled-pool"; occurrence: string }
  /** One printed hideaway instance on one permanent incarnation. Copied
   * triggers can add more than one card to the same linked pool (CR 607.3). */
  | { node: "hideaway-exiled-pool"; keyword: string; source: string; key: "source-incarnation";
      libraryOwner: string; lookCount: number; exile: "one-face-down"; remainder: "bottom-random";
      lookPermission: "controller-of-exiling-permanent"; membership: "still-in-exile"; cardinality: "many" }
  | { node: "total-power-condition"; scope: string; creatures: string; timing: "resolution";
      comparison: { op: "ge"; value: { amount: "fixed"; value: number } } }
  | { node: "damage-history-condition"; scope: string; opponentsOf: string; timing: "resolution"; during: "this-turn";
      comparison: { op: "ge"; value: { amount: "fixed"; value: number } }; quantifier: "any-one-opponent" }
  /** One grant created per resolution that applies as each later eligible spell is cast. */
  | { node: "cast-keyword-grant"; scope: string; created: "resolution"; duration: "this-turn"; keyword: "cascade" }
  | { node: "future-cast-spells"; grant: string; caster: string; from: { zone: "hand"; owner: string };
      timing: "as-cast"; cardinality: "many" }
  | { node: "runtime-player"; scope: string; role: "static-source-controller" | "activator" | "spell-controller" | "trigger-controller" | "prospective-caster"
      | "special-action-player" | "targeting-player" | "declaring-player" | "card-owner" | "entering-controller" | "replacement-source-controller-or-owner" }
  /** One keyword instance of this ability line, with the kinds of ability it
   * represents (CR 702); `you` is the player its printed parameters refer to. */
  | { node: "keyword-ability"; scope: string; keyword: string; abilities: KeywordAbilityKind[]; you: string; protectionFrom?: "everything" }
  /** "opponents" and "other-players" are the players relative to `of` (a "you");
   * "defending" is the player the attacking creature `of` attacks (CR 508.5). */
  | { node: "derived-player"; scope: string; role: "owner" | "controller" | "opponents" | "other-players" | "defending"; of: string; information?: "current-or-last-known" }
  /** The object or player the source permanent is attached to (CR 301.5f, 303.4m). */
  | { node: "derived-object"; scope: string; role: "attached-to"; of: string }
  /** The source of an ability an effect grants: each object of the bound
   * group `of` (`each`), or the object the effect creates (CR 113.10). */
  | { node: "recipient"; scope: string; of?: string; each?: boolean; created?: true;
      emblem?: { owner: string; controller: string; zone: "command" } }
  | { node: "selection"; scope: string; expression: ObjectRef | PlayerRef; cardinality: BindingCardinality;
      /** Explicit player binding for the announced target when established. */
      chooser?: string;
      countChoice?: string;
      perMember?: string;
      timing: BindingPhase | "stack-announcement" | "target-legality"; selection: "target" | "evaluate-reference" | "event-participant-filter" | "resolution-choice" }
  /** `optional`: performed only if `decider` chooses to (CR 608.2d); a missing
   * decider is unresolved. `repeated`: scaled by a "for each" count. */
  | { node: "event"; scope: string; operation: string; inputs: string[]; availability: "when-performed";
      optional?: true; decider?: string; repeated?: "for-each"; conditional?: true; condition?: string;
      /** A resolution choice exists only when this preceding result was produced. */
      requiresResult?: string }
  /** "If you do" / "If you don't": the optional instruction at path `decision`
   * of the preceding sentence (CR 608.2c-d). */
  | { node: "decision-link"; scope: string; condition: "you-do" | "you-dont"; decision: string }
  /** Count eligible symbols of the looked-at object before offering its reveal. */
  | { node: "colored-mana-symbol-count"; scope: string; card: string; guardedEvent: string;
      comparison: Extract<Effect, { effect: "look-top-may-reveal-for-mana" }>["revealIf"] }
  /** "If it was kicked": the kicker ability printed on the same object the
   * condition refers to (CR 702.33e, 607.2). */
  | { node: "linked-ability"; scope: string; condition: "kicked"; ability: string }
  /** "Destroyed this way", "milled this way": the objects the one earlier
   * instruction `event` of the same ability acted on (CR 608.2c). */
  | { node: "event-link"; scope: string; action: string; event: string }
  /** "The rest": the objects of `pool` that the selection `excluding` did
   * not take (CR 608.2c). */
  | { node: "remainder"; scope: string; pool: string; excluding: string }
  /** Whichever of `of` the event that occurred established: one per
   * alternative trigger event ("blocks or becomes blocked by a creature",
   * CR 603.2). */
  | { node: "alternatives"; scope: string; of: string[] }
  /** The would-be move of the source before applying any replacement (CR 400.6). */
  | { node: "zone-change-proposal"; scope: string; object: string; from: "anywhere"; to: { zone: "graveyard"; owner: string } }
  /** The source's own proposed entry under its would-be controller (CR 614.12). */
  | { node: "battlefield-entry-proposal"; scope: string; object: string; to: "battlefield"; controller: string }
  /** The battlefield at the choice point, with entry history for each current incarnation (CR 110.1, 400.7, 614.12a). */
  | { node: "entry-copy-model-choice"; scope: string; proposal: string; chooser: string; timing: "before-entry";
      selection: "non-target"; count: "zero-or-one"; zone: "battlefield"; filter: ObjectFilter; historyIdentity?: "current-incarnation" }
  /** The same proposed entrant enters with the model's copiable values if chosen (CR 707.2, 707.5). */
  | { node: "entry-copy-replacement"; scope: string; entry: string; model: string; timing: "as-enters";
      values: "model-copiable-values"; onDecline: "leave-entry-unchanged";
      changes?: EntryCopy["changes"]; duration?: EntryCopy["duration"]; exceptions?: EntryCopy["entryExceptions"]; entryState?: EntryCopy["entry"] }
  /** Reveal in the original zone, then move into the specified library and
   * shuffle it. The shuffle still occurs if the library move is redirected (CR 701.24c). */
  | { node: "reveal-and-shuffle-replacement"; scope: string; replaces: string; occurrence: "each";
      reveal: { actor: string; object: string }; shuffleInto: { actor: string; object: string; library: { zone: "library"; owner: string } } }
  | { node: "event-result"; event: string; result: "objects" | "quantity" | "player"; cardinality: BindingCardinality;
      description?: ObjectFilter;
      /** The result belongs to an earlier result's pool, if produced. */
      subsetOf?: string;
      /** Result sets can be empty when an instruction is impossible or replaced. */
      availability: "if-produced"; identity: "same-objects" | "zone-successors" | "new-objects" | "measured-quantity" }
  /** CR 607.1c/607.2d, 608.2d, 614.12a: a value belongs to one
   * resolution or is stored for the entering source incarnation. */
  | { node: "value-choice"; scope: string; event: string; chooser: string; category: ValueChoice["category"]; within?: "permanent-types";
      options?: Extract<ValueChoice, { category: "type" }>["options"];
      lifetime: { kind: "resolution" } | { kind: "mode-occurrence"; modal: string } | { kind: "source-incarnation"; source: string; lookup: "current-or-last-known" } }
  | { node: "player-state-condition"; scope: string; who: string;
      query: "designation" | "completed-dungeon" | "opponent-count" | "turn-of" }
  | { node: "equip-target-color-discount"; scope: string; chooser: string; source: string; timing: "after-target-announcement" }
  | { node: "repeat-hand-placement"; scope: string; place: string; draw: string; continueWhen: "placement-performed" }
  | { node: "target-count"; scope: string; chooser: string; minimum: number; maximum?: number; maximumNumber?: string }
  /** CR 601.2d: as its targets are announced, the chooser divides the fixed
   * total among them, each receiving at least `minimumEach`. */
  | { node: "target-division"; scope: string; chooser: string; event: string; targets: string; total: number; minimumEach: 1 }
  | { node: "casting-exile-commitment"; scope: string; caster: string; filter: ObjectFilter;
      minimum: 0; discountPerCard: number }
  | { node: "resolution-count"; scope: string; chooser: string; minimum: number; maximum?: number;
      operation: "discard" | "sacrifice" | "move-from-hand" | "search" | "draw" | "cast-from-exile" }
  | { node: "recipient-member"; scope: string; group: string; use: "damage-amount" }
  | { node: "devotion"; scope: string; who: string; colors: Extract<Amount, { amount: "devotion" }>["colors"] }
  /** The proposed event and the scalar change made when this replacement applies. */
  | { node: "event-participant"; scope: string; event: string; cardinality: "one";
      role: "proposed-damage-recipient" | "proposed-damage-source" | "prevented-damage-recipient" | "proposed-move-object" | "spell-cast-by-permission" | "proposed-permission-spell" }
  | { node: "exile-replacement"; scope: string; object: string; filter?: string;
      from: "battlefield" | "anywhere"; originalDestination: "graveyard"; exileEvent: string; reflexiveTrigger?: string;
      graveyardOwner?: string; control?: { differentFrom: string; information: "before-move" };
      damageHistory?: { controller: string; sourceController: "at-damage"; recipient: string; window: "this-turn" } }
  | { node: "linked-exile-life-cost"; scope: string; pool: string; player: string; spell: string;
      availability: "during-permitted-players-turn"; timing: "cost-determination";
      payment: "life-equal-to-proposed-spell-mana-value"; landPlay: "normal-rules" }
  | { node: "damage-prevention-pair"; scope: string; source: string; recipient: string; predicate: string;
      sourceFilter?: string; recipientFilter?: string; sourceInformation: "current-or-last-known" }
  | { node: "captured-move-replacement"; scope: string; created: string; exile: string;
      match: Extract<ReplacementInstruction["event"], { event: "move-zone" }>;
      tracked: { kind: "one-object"; object: string } | { kind: "cast-permission"; grant: string; member: string }
        | { kind: "matching-before-move"; filter: string }; duration?: "this-turn" }
  | { node: "replacement-return"; scope: string; exile: string; objects: string; delayed: string; returnEvent: string;
      schedule: "per-exiled-permanent-at-next-end-step"; membership: "exact-exile-card-incarnations"; controller: "each-card-owner" }
  | { node: "next-creature-entry-counter"; scope: string; created: string; trigger: string; spell: string; modifier: string;
      window: "this-turn"; move: "specific-stack-incarnation-resolving-to-battlefield"; counter: "+1/+1"; count: 1 }
  | { node: "performed-action-trigger"; scope: string; event: string; outcome: "actually-exiled" }
  /** CR 615.5: apply the follow-up to the same recipient using the amount actually prevented. */
  | { node: "prevention-outcome"; scope: string; targetFilter: string; kind: "all" | "combat" | "noncombat";
      prevented: string; recipient: string; followup: string }
  | { node: "quantity-replacement"; scope: string; source: string; match: ReplacementInstruction["event"];
      change: Extract<ReplacementInstruction["replace"], { replacement: "modify-quantity" }>["change"];
      unit: "token-specification" | "mana-specification" | "damage-source-and-recipient" | "life-source-and-recipient" | "counter-kind-and-recipient";
      operands: string[] }
  /** CR 122.2, 122.6, 603.2c: a matched counter change, grouped by its one recipient. */
  | { node: "counter-change-match"; scope: string; event: string;
      pattern: Extract<Trigger, { trigger: "counter-change" }>; recipients: string[]; actor?: string;
      gate?: { zone: "exile"; timing: "matching-event" } }
  /** CR 115.7, 707.10: operate on one exact stack object or on copies as they are created. */
  | { node: "stack-instruction"; scope: string; subject: string; chooser: string;
      action: { kind: "copy"; event: string; count: number; retarget: boolean }
        | { kind: "change-targets"; scope: "all" | "one" | "any" }
        | { kind: "choose-new-targets"; timing: "resolution" | "copy-creation"; copyEvent?: string } }
  | { node: "conditional-copy-count"; scope: string; event: string; subject: string; condition: Condition;
      counts: { original: number; replacement: number } }
  | { node: "alternate-target-copy"; scope: string; event: string; subject: string; caster: string;
      targetPolicy: NonNullable<Extract<Effect, { effect: "copy-stack-object" }>["fixedTargets"]> }
  | { node: "per-player-copies"; scope: string; event: string; subject: string; group: string; member: string;
      retarget: "optional-per-copy" }
  | { node: "permanent-spell-copy-grant"; scope: string; copy: string; applies: "permanent-spell-and-resulting-token"; cardinality: "one" }
  | { node: "copy-return-original"; scope: string; original: string; copy: string; copyEvent: string;
      returnEvent: string; owner: string; targetOption: string; whenNoCopy: "return-original" }
  | { node: "paired-random-reselection"; scope: string; event: string; original: string; copy: string;
      chooser: string; exclude: [string, string]; unit: "target-slot"; whenUnavailable: "keep"; whenNoCopy: "reselect-original" }
  | { node: "opponent-copy-count"; scope: string; original: string; opponents: string; member: string; opponentEvent: string;
      participants: string; final: string; controller: string; extra: 1; order: "apnap-opponents-then-controller";
      opponentPlacement: "before-next-opponent"; retarget: "optional-per-copy" }
  | { node: "contingent-copy-grant"; scope: string; event: string; copy: string; chosenTargets: string;
      affected: string; keyword: "wither"; copyFrom: "current-or-last-known"; grantTo: "still-on-stack" }
  | { node: "random-target-reselection"; scope: string; event: string; subject: string; guard: "single-target-intervening-if" }
  | { node: "face-down-exile-look"; scope: string; viewer: string; card: string; event: string;
      duration: Extract<Effect, { effect: "grant-look-permission" }>["duration"]; continuedUntil: "leaves-exile-or-shuffled" }
  | { node: "private-library-look"; scope: string; viewer: string; libraryOwner: string;
      refresh: "after-announcement-or-special-action" }
  | { node: "text-choice"; scope: string; subject: string; chooser: string;
      replacements: Extract<Effect, { effect: "change-text" }>["replacements"] }
  | { node: "characteristic-subject"; scope: string; role: "evaluated-candidate" | "affected-member"; of?: string;
      timing: BindingPhase | "stack-announcement" | "target-legality" }
  | { node: "characteristic-predicate"; scope: string; subject: string; test: CharacteristicRelation;
      operand: { kind: "object"; object: string; information: "current" | "current-or-last-known" }
        | { kind: "matching-set"; filter: ObjectFilter; quantifier: "any" | "every" }
        | { kind: "linked-exile-names"; pool: string; quantifier: "any" };
      timing: BindingPhase | "stack-announcement" | "target-legality" | "intervening-if" }
  | { node: "common-name-subset"; scope: string; controller: string; filter: ObjectFilter; minimum: number }
  | { node: "static-rule"; scope: string; rule: Extract<StaticEffect, { static: "prohibit" | "untap-during-step" | "untap-step-limit" }>;
      operands: string[] }
  | { node: "keep-unspent-mana"; scope: string; player: string; symbol: string }
  /** CR 608.2c, 609.3, 701.13, 701.20: one ordered scan that can exhaust its library. */
  | { node: "library-scan"; scope: string; actor: string; owner: string; event: string;
      operation: "reveal" | "exile"; filter: ObjectFilter; count: number;
      pool: string; matches: string }
  | { node: "linked-exile-ledger"; scope: string; source: string; writers: { event: string; result: string }[];
      membership: "still-in-exile"; identity: "source-incarnation"; cardinality: "many" }
  | { node: "cast-after-exile-payment"; scope: string; caster: string; spell: string; payment: string; actualExile: string;
      ledger: string; pool: string; cast: string; check: "mandatory-exile-payment-started" }
  | { node: "shared-exile-play"; scope: string; ledger: string; player: string; maximum: 1;
      alternatives: { action: "play-land" | "cast-spell"; use: string; player: string; refill: string; mana?: "any-color" }[];
      period: "actual-turn-per-player-and-source-incarnation"; commit: "completed-use-of-this-permission";
      refillController: "source-controller-at-use"; candidate: "proposed-face-for-action" }
  | { node: "linked-scan-cleanup"; scope: string; ledger: string; caster: string; spell: string; match: string;
      cast: string; cleanup: string; destination: "owners-libraries-random" }
  | { node: "grouped-library-scan"; scope: string; event: string; group: string; member: string; pool: string; matches: string;
      matching: ObjectFilter; maximumMatchesPerLibrary: 1 }
  | { node: "excluded-scan-cast"; scope: string; chooser: string; opponent: string; exclude: string; pool: string; cast: string;
      maximum: 2; order: "one-legal-cast-at-a-time" }
  | { node: "card-copy"; scope: string; event: string; card: string; chooser: string; result: string; count: number }
  | { node: "group-instruction"; scope: string; event?: string;
      execution: { kind: "draw"; players: string[]; amount: number; order: "apnap" }
        | { kind: "for-each"; domain: string; member: string; membership: "announcement" | "resolution"; processing: "instruction-wise"; ordering?: "each-owner-private-simultaneous" } }
  /** The targets that are still legal when this set of per-player targets resolves. */
  | { node: "opponent-discard-or-life"; scope: string; opponents: string; payer: string; payment: string; loss: string;
      cards: number; life: number; lossWhen: "did-not-pay"; order: "apnap-private-choices-then-discards-then-losses" }
  | { node: "optional-action-participants"; scope: string; event: string; group: string;
      outcome: "did-not-pay"; activation: string; cardinality: "many" }
  | { node: "optional-matching-sacrifice"; scope: string; controllerPayment: string; original: string; opponents: string;
      opponentPayment: string; nonpayers: string; lose: string; life: number; draw: string; cards: number;
      requireOriginalPayment: true }
  | { node: "exiled-creature-controllers"; scope: string; event: string; information: "before-move"; cardinality: "many" }
  | { node: "actual-library-searchers"; scope: string; searches: string[]; cardinality: "many" }
  | { node: "coalesced-library-search"; scope: string; grouping: "same-player-library-before-shuffle";
      actualSearchers: string; shuffle: string;
      stages: { search: string; actor: string; found: string; place: string; entersTapped?: true;
        quota: { kind: "fixed"; count: 1 } | { kind: "optional-per-opponent"; group: string }
          | { kind: "per-last-controller" | "one-per-actual-searcher"; source: string } }[] }
  | { node: "replacement-search-result"; scope: string; original: string; replacement: string;
      condition: Condition; cardinality: "many" }
  | { node: "simultaneous-search-partition"; scope: string; pool: string; selected: string; remaining: string;
      battlefield: string; hand: string; chooser: string; count: "one-if-found"; order: "choose-before-both-moves" }
  | { node: "existing-counter-kind"; scope: string; chooser: string; member: string; event: string; count: 1;
      placement: "optional-for-each-member"; ordering: "choose-all-then-place" }
  | { node: "multiplayer-choice"; scope: string; event: string; group: string; member: string;
      selector: string; result: string; startWith?: string; choose: "one-per-permanent-type" | "printed-count" }
  | { node: "unchosen-permanents"; scope: string; chosen: string; member?: string; filter: ObjectFilter;
      exclude: "all-player-choices" | "own-choices"; cardinality: "many" }
  | { node: "intervening-condition-pool"; scope: string; filter: ObjectFilter; cardinality: "many";
      timing: "trigger-check-and-resolution" }
  | { node: "named-vote"; scope: string; event: string; start: string; names: string[] }
  | { node: "vote-outcome"; scope: string; event: string; choice: string; comparison: "strict-majority" }
  /** One resolution-local choice per opponent, with a shared reveal. */
  | { node: "secret-choice-reveal"; scope: string; event: string; choosers: string; controller: string;
      options: [string, string]; collection: "private-apnap"; count: "one-per-chooser"; reveal: "simultaneous" }
  | { node: "secret-choice-outcome"; scope: string; revealed: string; recipients: string;
      damage: { amount: number; timing: "simultaneous"; sourceInformation: "current-or-last-known" };
      when: { kind: "unanimous"; choice: string; unless?: string } | { kind: "otherwise"; unless: [string, string] } }
  | { node: "secret-choice-subset"; scope: string; revealed: string; chose: string; cardinality: "many" }
  | { node: "linked-pool-condition"; scope: string; pool: string; comparison: "nonempty"; timing: "trigger-check-and-resolution" }
  | { node: "exile-play-lifetime"; scope: string; exile: string; player: string; duration: "while-that-object-is-exiled" | "end-of-turn" | "your-next-end-step";
      visibility?: "turn-face-up-before-announcement-unless-cast-face-down"; requiresDecision?: string }
  | { node: "actual-destination-count"; scope: string; event: string; result: string; destination: "graveyard" }
  | { node: "weighted-modal"; scope: string; chooser: string; maximum: number; weights: number[]; repeat: boolean;
      timing: "stack-announcement"; order: "printed-mode-order" }
  | { node: "commander-set"; scope: string; owner: string; cardinality: "many" }
  | { node: "commander-mana-rider"; scope: string; manaEvent: string; commanderSet: string; trigger: string;
      comparison: "shares-creature-type-with-any"; evaluate: "when-mana-spent-on-cast"; occurrence: "each-mana-produced" }
  | { node: "numeric-roll"; scope: string; event: string; roller: string; sides: number; result: string; stage: "final-kept-result" }
  | { node: "random-aura-attachment"; scope: string; event: string; aura: string; opponentOf: string; exclude: string; cardinality: "one" }
  | { node: "trigger-graveyard-view"; scope: string;
      alternatives: { kind: "dies" | "leaves" | "enters"; event: string; previous?: string; successor?: string }[];
      successorZone: "graveyard"; comparison?: NonNullable<ObjectFilter["implicitComparison"]>; information?: "before-move" }
  | { node: "entering-kicker-x"; scope: string; source: string; kicker: string; announced: string }
  | { node: "bounded-payment-amount"; scope: string; payment: string; payer: string; minimum: 0; maximum: string }
  | { node: "destroyed-permanents"; scope: string; event: string; destination: "graveyard";
      information: "before-move"; cardinality: "many" }
  | { node: "controller-search"; scope: string; exile: string; player: string; decision: string;
      search: string; entry: string; shuffle: string }
  | { node: "result-union"; scope: string; from: string[]; cardinality: "many" }
  | { node: "shared-land-type-search"; scope: string; chooser: string; library: string; event: string;
      filter: ObjectFilter; maximum: 2; characteristic: "land-type" }
  | { node: "relative-player-group"; scope: string; base: string; cardinality: "many";
      filter: { kind: "attacking" | "other-than"; player: string } }
  | { node: "filtered-result-group"; scope: string; pool: string; filter: ObjectFilter; cardinality: "many" }
  | { node: "eligible-target-group"; scope: string; template: string; member: string; cardinality: "many" }
  | { node: "sequential-cast"; scope: string; event: string; pool: string; caster: string; limit: string }
  | ({ node: "opponent-group-damage"; scope: string; event: string; players: string } &
      ({ creatures: string; planeswalkers?: never } | { creatures?: never; planeswalkers: string }))
  | { node: "token-fight-pairing"; scope: string; event: string; creation: string; tokens: string; originals: string; chooser: string;
      pairing: "distinct-one-to-one-omit-unmatched"; combatants: "same-live-creatures"; order: "simultaneous" }
  | { node: "source-dependent-damage"; scope: string; event: string; group: string; sourceAndRecipient: string;
      characteristic: "power"; timing: "simultaneous" }
  | { node: "instruction-member"; scope: string; group: string; domain: "player" | "object" }
  | { node: "event-aggregate"; scope: string; event: string; measure: "damage-dealt" | "life-lost" }
  | { node: "linked-exile-pool"; scope: string; source: string; producer: string; event: string;
      membership: "still-in-exile"; identity: "source-incarnation"; cardinality: "many" }
  | { node: "linked-card-copies"; scope: string; pool: string; eligible: string; event: string; caster: string; result: string }
  | { node: "cast-linked-copies"; scope: string; event: string; copies: string; caster: string; choice: "optional-per-copy" }
  | { node: "linked-exile-filter"; scope: string; pool: string }
  | { node: "blight-payment"; scope: string; payer: string; count: number }
  | { node: "action-cost-condition"; scope: string; event: string; action: "sacrifice" | "discard" | "tap" }
  | { node: "value-exchange"; scope: string; subjects: [string, string]; event: string; operation: "life-and-toughness" }
  | { node: "targeting-permission"; scope: string; controller: string; affected: string[]; evaluation: "each-target-legality-check";
      duration: Extract<Effect, { effect: "targeting-as-though" }>["duration"] }
  | { node: "scheduled-phase"; scope: string; phase: Extract<Trigger, { trigger: "phase-in-turn" }>["phase"]; turn: "next" }
  /** The matched step occurrence, and the attachment criterion at match time. */
  | { node: "first-upkeep-start"; scope: string; event: string; enchantedPlayer: string;
      occurrence: "first-actual-upkeep-of-turn"; attachmentAt: "trigger-match" }
  | { node: "upkeep-insertion"; scope: string; event: string; player: string; after: string;
      count: Amount; placement: "same-phase"; order: "most-recent-first" }
  | { node: "skip-draw-step"; scope: string; who: string }
  | { node: "cast-mana-paid"; scope: string; occurrence: string; measure: "total" }
  | { node: "cast-history-exclusion"; scope: string; occurrence: string; caster: string }
  | { node: "activation-permission"; scope: string; source: string; actors: "any-player" | "opponents-of-source-controller" }
  | { node: "payment-alternatives"; scope: string; payer: string; options: Extract<Cost, { cost: "choice" }>["options"] }
  | { node: "class-level"; scope: string; source: string; payer: string; level: number; requiresLevel: number;
      timing: "sorcery"; abilities: string[] }
  | { node: "attack-declaration"; scope: string; event: string; actor: string; defender?: string; attackers?: string;
      filter?: ObjectFilter; count?: Extract<Trigger, { trigger: "player-attacks" }>["count"] }
  | { node: "attack-declaration-count"; scope: string; attackers: string; defendingPlayer: string;
      defendingPlaneswalkers: string; minimum: number; lookup: "current-or-last-known"; timing: "trigger-check-and-resolution" }
  | { node: "entrant-control-exchange"; scope: string; entrant: string; chooser: string; target: string; event: string;
      targetRestrictions: "recheck-at-resolution"; execution: "both-live-and-different-controllers" }
  | { node: "damage-history-draw"; scope: string; event: string; player: string; target: string; departure: string;
      window: "this-turn"; sourceController: "at-damage"; order: "active-player-then-turn-order" }
  | { node: "attack-payment"; scope: string; payer: string; eligible: string;
      rule: Extract<Extract<StaticEffect, { static: "combat-rule" }>["rule"], { rule: "declaration-cost" }> }
  | { node: "additional-stack-copy"; scope: string; actor: string; mayRetarget: boolean }
  | { node: "defined-number"; scope: string; definition: Amount }
  /** X chosen and announced by `chooser` as the spell was cast or the ability
   * activated (CR 107.3a, 107.3k), or, for a permanent's own entry, as the
   * spell it was cast (CR 107.3m). */
  | { node: "announced-number"; scope: string; chooser: string; announcement: "cast" | "activation" | "entering-spell"; constraints?: VariableConstraint[] }
);

export type ReferenceBinding = {
  path: string;
  expression: ReferenceExpression;
  rules: string[];
} & (
  | { binding: "bound"; target: string }
  | { binding: "unresolved"; code: BindingDiagnosticCode; candidates: string[] }
);

export type BindingDiagnosticCode =
  | "invalid-source" | "missing-face" | "unparsed-face" | "face-mismatch"
  | "missing-antecedent" | "ambiguous-antecedent" | "cardinality-mismatch"
  | "missing-target-identity" | "ambiguous-target-identity" | "unsupported-scope" | "unsupported-reference"
  | "unsupported-number-binding" | "duplicate-number-definition" | "unresolved-dependency";

export interface BindingDiagnostic {
  code: BindingDiagnosticCode;
  path: string;
  message: string;
  rules: string[];
}

/** A rules-engine capability a card needs that is not reference binding: its
 * references resolve, but executing it needs the engine to provide this.
 * - protection-from-everything: enforce all protections against all sources (CR 702.16j).
 * - graveyard-reveal-and-shuffle: replace the proposed move with a reveal and a move-and-shuffle (CR 400.6, 614.6, 701.24c).
 * - event-history: a record of what happened ("if you attacked this turn",
 *   "if this spell was cast from a graveyard", "life you gained this turn"),
 *   including the spell a permanent was (CR 400.7d, 608.2i).
 * - damage-prevention: shields and redirections watching future damage
 *   events (CR 609.7, 614.9, 615).
 * - characteristic-layers: characteristic changes applied in the layer system,
 *   with the affected set fixed when the effect begins (CR 611.2c, 613).
 * - cascade-resolution: a choice bound to one resolving cascade and its exiled pool (CR 702.85).
 * - cast-keyword-grant: a resolution-created grant applied during later casts (CR 601.2, 611.2, 702.85).
 * - permanent-spell-copy: a stack copy that becomes a token permanent as it resolves (CR 707.10f).
 * - entry-copy-replacement: choose from current entrants' battlefield history, then apply copiable values during entry (CR 614.12, 707.2, 707.5).
 * - demonstrate: cast-triggered copies for the caster and chosen opponent (CR 702.144a).
 * - reveal-and-cast: resolve the scan, random bottom placement and immediate cast (CR 608.2c–g). */
export type EngineFeature = "protection-from-everything" | "graveyard-reveal-and-shuffle"
  | "event-history" | "damage-prevention" | "characteristic-layers" | "cascade-resolution"
  | "repeatable-modal-selection" | "cast-keyword-grant" | "permanent-spell-copy" | "entry-copy-replacement" | "demonstrate" | "reveal-and-cast" | "hideaway-resolution" | "linked-exile-play" | "look-top-may-reveal-for-mana" | "quantity-replacement" | "devotion" | "counter-change-event" | "stack-instruction" | "text-change" | "private-library-look" | "recipient-dependent-damage" | "characteristic-predicate" | "static-rule" | "keep-unspent-mana" | "library-scan" | "card-copy" | "group-instruction" | "event-aggregate" | "monstrosity-event" | "emblem" | "linked-exile-pool" | "intrinsic-payment" | "action-cost-condition" | "value-exchange" | "targeting-permission" | "phase-scheduling" | "counter-destination" | "activation-permission" | "class-level" | "attack-declaration" | "attack-payment" | "zone-replacement" | "source-dependent-damage" | "multi-library-exile" | "sequential-cast" | "private-exile-look" | "exile-and-search-results" | "shared-land-type-search" | "opponent-group-damage" | "controller-search" | "bounded-payment-amount" | "intervening-condition-pool" | "casting-exile-discount" | "multiplayer-choice" | "linked-card-copies" | "repeat-hand-placement" | "optional-look-remainder" | "equip-color-discount" | "commander-mana-rider" | "numeric-roll" | "random-aura-attachment" | "vote-outcome" | "actual-destination-count" | "weighted-modal-selection" | "existing-counter-kind" | "quoted-source-reference" | "entrant-control-exchange" | "search-result-partition" | "coalesced-library-search";
export interface EngineFeatureNeed {
  feature: EngineFeature;
  path: string;
  message: string;
  rules: string[];
}

/** Reference analysis is neither full semantic validation nor engine support.
 * referencesBound: every reference inspected here resolves, with no
 * diagnostics. engineFeatures: what the rules engine must provide beyond that.
 * fullyBound: references bound and no engine feature needed. Unknown scope
 * behavior is diagnosed and prevents both, even with zero refs. */
export interface CardBindingAnalysis {
  artifactKind: "oracle-reference-binding";
  schemaVersion: 1;
  executable: false;
  identity: string;
  rules: { edition: "2026-09-25"; url: string };
  nodes: BindingNode[];
  references: ReferenceBinding[];
  diagnostics: BindingDiagnostic[];
  counts: { obligations: number; bound: number; unresolved: number };
  referencesBound: boolean;
  engineFeatures: EngineFeatureNeed[];
  fullyBound: boolean;
}
