parser grammar MTGKeywordsParser;

options { tokenVocab=MTGLexer; }

// Keyword abilities rules
keywordAbility
    : simpleKeyword
    | parametricKeyword
    ;

simpleKeyword
    : FLYING
    | VIGILANCE
    | HASTE
    | TRAMPLE
    | DEATHTOUCH
    | LIFELINK
    | FIRST STRIKE
    | DOUBLE STRIKE
    | DEFENDER
    | REACH
    ;

parametricKeyword
    : ward
    | equip
    | kicker
    | evoke
    ;

ward
    : WARD cost
    ;

equip
    : EQUIP cost
    ;

kicker
    : KICKER cost
    ;

evoke
    : EVOKE cost
    ;
