parser grammar MTGKeywordsParser;

options { tokenVocab=MTGLexer; }

// Keyword abilities rules
keywordAbility
    : simpleKeyword
    | compoundKeyword
    | parametricKeyword
    ;

keywordAbilityList
    : keywordAbility ((COMMA (AND)? | AND)? keywordAbility)*
    ;

simpleKeyword
    : FLYING
    | VIGILANCE
    | HASTE
    | TRAMPLE
    | DEATHTOUCH
    | LIFELINK
    | DEFENDER
    | REACH
    | FLASH
    | SHROUD
    | HEXPROOF
    | INDESTRUCTIBLE
    ;

compoundKeyword
    : FIRST STRIKE
    | DOUBLE STRIKE
    ;

parametricKeyword
    : ward
    | equip
    | kicker
    | evoke
    | protection
    | hexproofFrom
    | enchant
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

protection
    : PROTECTION FROM (targetSelector | EACH? colorQualifier | EVERYTHING)
    ;

hexproofFrom
    : HEXPROOF FROM targetSelector
    ;

enchant
    : ENCHANT cardFilter
    ;
