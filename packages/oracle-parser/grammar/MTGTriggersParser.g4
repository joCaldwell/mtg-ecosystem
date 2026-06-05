parser grammar MTGTriggersParser;

options { tokenVocab=MTGLexer; }

// Trigger conditions
trigger
    : entersBattlefieldTrigger
    | upkeepTrigger
    | castingTrigger
    | deathTrigger
    ;

entersBattlefieldTrigger
    : WHEN selfQualifier ENTERS
    | WHEN selfQualifier ENTERS THE BATTLEFIELD
    | WHENEVER cardFilter ENTERS THE BATTLEFIELD
    | WHENEVER cardFilter ENTERS
    | WHEN cardFilter ENTERS THE BATTLEFIELD
    | WHEN cardFilter ENTERS
    ;

upkeepTrigger
    : AT BEGINNING OF controllerQualifier UPKEEP
    | AT BEGINNING OF EACH PLAYER UPKEEP
    ;

castingTrigger
    : WHENEVER targetSelector CASTS cardFilter
    | WHENEVER YOU CAST cardFilter
    ;

deathTrigger
    : WHENEVER targetSelector DIES
    | WHEN targetSelector DIES
    ;
