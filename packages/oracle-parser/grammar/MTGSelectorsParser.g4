parser grammar MTGSelectorsParser;

options { tokenVocab=MTGLexer; }

// Target and Selector rules
targetSelector
    : (targetQualifier
    | eachQualifier
    | allQualifier
    | selfQualifier
    | controllerQualifier
    | opponentQualifier
    | cardFilter
    | THIS) (controlSpec)?
    ;

controlSpec
    : YOU CONTROL
    | A? OPPONENT CONTROLS
    ;

targetQualifier : TARGET cardFilter | ANY TARGET ;
eachQualifier   : EACH cardFilter | EACH opponentQualifier ;
allQualifier    : ALL cardFilter ;
selfQualifier   : TILDE | ITS ;
controllerQualifier : YOU | YOUR ;
opponentQualifier   : OPPONENT | TARGET OPPONENT ;

// Composable filter loop
cardFilter
    : (A)? filterQualifier* cardType filterQualifier* (CARD)? filterQualifier* (locationSpec)? (controlSpec)?
    | (A)? filterQualifier+ (CARD)? (locationSpec)? (controlSpec)?
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
    | IDENTIFIER // Handles subtypes (Elf, Aura, etc.) dynamically
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
    : FROM (YOUR)? zone
    | IN (YOUR)? zone
    | ON THE BATTLEFIELD
    ;

cardType
    : CREATURE | PLAYER | PLANESWALKER | PERMANENT | SPELL | CARD | LAND | ARTIFACT | ENCHANTMENT
    ;

zone
    : BATTLEFIELD | GRAVEYARD | LIBRARY | HAND | EXILE
    ;
