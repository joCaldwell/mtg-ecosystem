parser grammar MTGParser;

options { tokenVocab=MTGLexer; }

// Import all modular sub-parsers
import MTGCostsParser,
       MTGSelectorsParser,
       MTGValuesParser,
       MTGModifiersParser,
       MTGTriggersParser,
       MTGEffectsParser,
       MTGKeywordsParser;

// Entry point rule
card : ability* EOF ;

ability
    : activatedAbility
    | triggeredAbility
    | staticAbility
    | spellEffect
    | keywordAbility
    | choiceAbility
    ;

activatedAbility : costList COLON effectList PERIOD ;
triggeredAbility : trigger COMMA effectList PERIOD ;
staticAbility    : staticEffect PERIOD ;
spellEffect      : effectList PERIOD ;

choiceAbility
    : chooseHeader choiceOptionList
    ;

chooseHeader
    : CHOOSE choiceCount (OR MORE_WORD)? EM_DASH
    | CHOOSE choiceCount (OR choiceCount)? (OR BOTH)? (OR MORE_WORD)? EM_DASH
    ;

choiceCount
    : ONE | TWO | THREE | BOTH
    ;

choiceOptionList
    : choiceOption+
    ;

choiceOption
    : BULLET ability
    ;

staticEffect
    : (AS LONG AS condition COMMA)? targetSelector GETS ptModifier duration?
    | (AS LONG AS condition COMMA)? targetSelector HAVE keywordAbility duration?
    | (duration COMMA)? targetSelector GETS ptModifier duration?
    | (duration COMMA)? targetSelector HAVE keywordAbility duration?
    | cardFilter YOU CONTROL GET ptModifier duration?
    | cardFilter YOU CONTROL HAVE keywordAbility duration?
    ;

effectList
    : effect (COMMA effect)*
    | effect (COMMA? (IF | UNLESS) condition)?
    ;
