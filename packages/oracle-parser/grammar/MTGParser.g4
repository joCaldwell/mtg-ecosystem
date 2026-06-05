parser grammar MTGParser;

options { tokenVocab=MTGLexer; }

// Import all modular sub-parsers
import MTGCostsParser,
       MTGSelectorsParser,
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
    ;

activatedAbility : costList COLON effectList PERIOD ;
triggeredAbility : trigger COMMA effectList PERIOD ;
staticAbility    : staticEffect PERIOD ;
spellEffect      : effectList PERIOD ;

staticEffect
    : cardFilter YOU CONTROL GET ptModifier duration?
    | cardFilter YOU CONTROL HAVE keywordAbility
    ;

effectList
    : effect (COMMA effect)*
    | effect (COMMA? condition)?
    ;
