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
    : (abilityWord EM_DASH)? (activatedAbility
    | triggeredAbility
    | staticAbility
    | spellEffect
    | keywordAbilityList
    | asAbility
    | quotedAbility)
    ;

abilityWord
    : nameWord+
    ;

sentence
    : (FOR targetSelector COMMA)? (THEN COMMA? | OTHERWISE COMMA?)? effectList (PERIOD)?
    | choiceAbility
    ;

activatedAbility : costList COLON sentence+ ;
triggeredAbility : trigger COMMA sentence+ ;
staticAbility    : staticEffect PERIOD? ;
spellEffect      : sentence+ ;

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
    : BULLET (abilityWord EM_DASH)? ability
    ;

staticModifier
    : (GETS | GET) ptModifier duration?
    | (HAVE | GAINS) gainableAbilityList duration?
    ;

staticModifierList
    : staticModifier (AND staticModifier)*
    ;

staticEffect
    : (AS LONG AS condition COMMA)? targetSelector staticModifierList
    | (duration COMMA)? targetSelector staticModifierList
    | cardFilter YOU CONTROL staticModifierList
    | targetSelector POWER (AND TOUGHNESS)? (IS | ARE EACH) EQUAL TO valueExpression
    ;

effectList
    : effect ((COMMA (THEN)? | THEN)? effect)*
    | effect (COMMA? (IF | UNLESS) condition)?
    ;

asHeader
    : AS targetSelector ENTERS (THE? BATTLEFIELD)? (COMMA)?
    | AS A? ADDITIONAL COST TO CAST THIS SPELL (COMMA)?
    ;

asAbility
    : asHeader sentence+
    ;
