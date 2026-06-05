parser grammar MTGSelectorsParser;

options { tokenVocab=MTGLexer; }

// Target and Selector rules
targetSelector
    : targetQualifier
    | eachQualifier
    | allQualifier
    | selfQualifier
    | controllerQualifier
    | opponentQualifier
    | cardFilter
    ;

targetQualifier : TARGET cardFilter | ANY TARGET ;
eachQualifier   : EACH cardFilter | EACH opponentQualifier ;
allQualifier    : ALL cardFilter ;
selfQualifier   : TILDE ;
controllerQualifier : YOU | YOUR ;
opponentQualifier   : OPPONENT | TARGET OPPONENT ;

// Composable filter loop
cardFilter
    : (A)? filterQualifier* cardType (CARD)? (locationSpec)?
    | (A)? filterQualifier+ (CARD)? (locationSpec)?
    ;

filterQualifier
    : statusQualifier
    | colorQualifier
    | supertypeQualifier
    | relationQualifier
    | negativeQualifier
    | IDENTIFIER // Handles subtypes (Elf, Aura, etc.) dynamically
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
