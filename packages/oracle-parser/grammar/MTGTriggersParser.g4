parser grammar MTGTriggersParser;

options { tokenVocab=MTGLexer; }

// Trigger conditions
trigger
    : entersBattlefieldTrigger
    | phaseTrigger
    | castingTrigger
    | deathTrigger
    | attackTrigger
    | blockTrigger
    | activationTrigger
    | targetTrigger
    ;

activationTrigger
    : (WHEN | WHENEVER) YOU ACTIVATE cardFilter
    ;

targetTrigger
    : (WHEN | WHENEVER) targetSelector BECOMES THE? TARGET OF A? cardFilter
    ;

triggerEvent
    : ENTERS
    | LEAVES
    | ATTACKS
    | ENTERS OR LEAVES
    | ENTERS OR ATTACKS
    | ENTERS THE BATTLEFIELD OR ATTACKS
    ;

entersBattlefieldTrigger
    : (WHEN | WHENEVER) (selfQualifier | cardFilter | targetSelector) triggerEvent (THE? BATTLEFIELD)?
    ;

castingTrigger
    : WHENEVER targetSelector CASTS cardFilter
    | WHENEVER YOU CAST cardFilter
    ;

deathTrigger
    : WHENEVER targetSelector DIES
    | WHEN targetSelector DIES
    ;

attackTrigger
    : (WHEN | WHENEVER) targetSelector ATTACKS (targetSelector (OR targetSelector)?)?
    | (WHEN | WHENEVER) YOU ATTACK
    ;

blockTrigger
    : (WHEN | WHENEVER) targetSelector BLOCKS (A? CREATURE | targetSelector)?
    | (WHEN | WHENEVER) targetSelector BECOMES BLOCKED (BY targetSelector)?
    | (WHEN | WHENEVER) targetSelector BLOCKS OR BECOMES BLOCKED (BY targetSelector)?
    ;

stepOrPhase
    : UPKEEP
    | DRAW STEP
    | UNTAP STEP
    | COMBAT
    | DECLARE ATTACKERS STEP
    | DECLARE BLOCKERS STEP
    | COMBAT DAMAGE STEP
    | END OF COMBAT STEP
    | END STEP
    | CLEANUP STEP
    | (FIRST | POSTCOMBAT | PRECOMBAT)? MAIN PHASE
    ;

turnOwnerQualifier
    : controllerQualifier
    | EACH PLAYER
    | EACH OPPONENT
    | EACH
    ;

turnOwnerSuffix
    : ON (controllerQualifier | EACH PLAYER | EACH OPPONENT) TURN
    ;

phaseTrigger
    : AT THE? BEGINNING OF (THE)? stepOrPhase turnOwnerSuffix
    | AT THE? BEGINNING OF turnOwnerQualifier stepOrPhase
    | AT THE? BEGINNING OF (THE)? stepOrPhase
    ;
