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
    ;

targetQualifier
    : TARGET cardFilter
    | ANY TARGET
    ;

eachQualifier
    : EACH cardFilter
    | EACH opponentQualifier
    ;

allQualifier
    : ALL cardFilter
    ;

selfQualifier
    : TILDE
    ;

controllerQualifier
    : YOU
    | YOUR
    ;

opponentQualifier
    : OPPONENT
    | TARGET OPPONENT
    ;

cardFilter
    : cardType
    | IDENTIFIER* cardType
    | cardType CARD
    | IDENTIFIER* cardType CARD
    ;

cardType
    : CREATURE
    | PLAYER
    | PLANESWALKER
    | PERMANENT
    | SPELL
    | CARD
    | LAND
    | ARTIFACT
    | ENCHANTMENT
    ;

zone
    : BATTLEFIELD
    | GRAVEYARD
    | LIBRARY
    | HAND
    | EXILE
    ;
