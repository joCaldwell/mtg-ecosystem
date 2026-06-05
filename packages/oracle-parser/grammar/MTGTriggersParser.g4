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
    ;

upkeepTrigger
    : AT BEGINNING OF controllerQualifier UPKEEP
    | AT BEGINNING OF EACH PLAYER UPKEEP
    ;

castingTrigger
    : WHENEVER targetSelector CASTS A SPELL
    | WHENEVER YOU CAST A SPELL
    ;

deathTrigger
    : WHENEVER targetSelector DIES
    ;
