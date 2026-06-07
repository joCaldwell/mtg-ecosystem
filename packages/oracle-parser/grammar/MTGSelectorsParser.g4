parser grammar MTGSelectorsParser;

options { tokenVocab=MTGLexer; }

// Target and Selector rules
possessivePronoun : ITS | THEIR | YOUR ;

pronounSelector
    : IT
    | THEY
    | THEM
    | ITSELF
    | THEMSELVES
    ;

relativeSelector
    : (THAT | THOSE) cardFilter
    ;

nameWord
    : IDENTIFIER
    | STRIKE
    | OF
    | AND
    | WITH
    | A
    | THE
    | TO
    | FROM
    | IN
    | ON
    | ONE
    | TWO
    | THREE
    | BOTH
    | OR
    | STEP
    | PHASE
    | MAIN
    | CLEANUP
    | PRECOMBAT
    | POSTCOMBAT
    | DECLARE
    | ATTACKERS
    | BLOCKERS
    | ADD
    | COLOR
    | TYPE
    | NAME
    | LEAVES
    | BY
    | ADDITIONAL
    | COST
    | REVEAL
    | MAY
    | DONT
    | ISNT
    | HEXPROOF
    | PROTECTION
    | FLASH
    | SHROUD
    | INDESTRUCTIBLE
    | ENCHANT
    | EVERYTHING
    | AMPERSAND
    ;

targetSelector
    : targetSelectorBase ((OR | AND) targetSelectorBase)*
    ;

targetSelectorBase
    : (targetQualifier
    | eachQualifier
    | allQualifier
    | selfQualifier
    | controllerQualifier
    | opponentQualifier
    | pronounSelector
    | relativeSelector
    | cardFilter
    | THIS) (controlSpec)?
    ;

controlSpec
    : YOU CONTROL
    | opponentQualifier CONTROLS
    ;

targetQualifier
    : (UP TO)? (valueOrWord | X_VAR)? TARGET cardFilter
    | ANY TARGET
    ;
eachQualifier   : EACH cardFilter | EACH opponentQualifier ;
allQualifier    : ALL cardFilter ;
selfQualifier   : TILDE | ITS ;
controllerQualifier : YOU | YOUR ;
opponentQualifier   : (A | TARGET)? OPPONENT ;

// Composable filter loop
cardFilter
    : (A | UP TO)? (valueOrWord | X_VAR)? filterQualifier* cardType filterQualifier* (CARD)? filterQualifier* (locationSpec)? (controlSpec)? (relativeClause)? (OF targetSelector)?
    | (A | UP TO)? (valueOrWord | X_VAR)? filterQualifier+ (CARD)? (locationSpec)? (controlSpec)? (relativeClause)? (OF targetSelector)?
    ;

filterQualifier
    : baseQualifier (OR baseQualifier)*
    ;

baseQualifier
    : statusQualifier
    | colorQualifier
    | supertypeQualifier
    | relationQualifier
    | negativeQualifier
    | propertyQualifier
    | TOKEN
    | THIS
    | nameWord // Handles subtypes (Elf, Aura, etc.) dynamically
    ;

propertyQualifier
    : (WITH)? (POWER | TOUGHNESS | MANA VALUE) valueExpression (OR (GREATER | LESS) (THAN)?)?
    ;

statusQualifier    : TAPPED | UNTAPPED | ATTACKING | BLOCKING ;
colorQualifier     : WHITE | BLUE | BLACK | RED | GREEN | COLORLESS | MULTICOLORED | NON_COLOR ;
supertypeQualifier : LEGENDARY | BASIC | SNOW | WORLD ;
relationQualifier  : OTHER | ANOTHER ;
negativeQualifier  : NONLAND | NONCREATURE | NONBASIC | NONTOKEN | NONARTIFACT | NONENCHANTMENT ;

locationSpec
    : (ONTO | ON) THE BATTLEFIELD
    | INTO (YOUR | A | ANY)? zone
    | FROM (YOUR | A | ANY)? zone
    | IN (YOUR | A | ANY)? zone
    ;

cardType
    : cardTypeBase (OR cardTypeBase)*
    ;

cardTypeBase
    : CREATURE | PLAYER | PLANESWALKER | PERMANENT | SPELL | CARD | LAND | ARTIFACT | ENCHANTMENT | ABILITY
    ;

relativeClause
    : THAT (TARGET | TARGETS) (ONLY)? (valueOrWord)? targetSelector
    | THAT (TARGET | TARGETS) (ONLY)? (valueOrWord) OR MORE_WORD targetSelector
    | THAT ISNT A? (MANA)? ABILITY
    | THAT SHARE A COLOR
    ;

zone
    : BATTLEFIELD | GRAVEYARD | LIBRARY | HAND | EXILE
    ;
