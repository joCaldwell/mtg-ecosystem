parser grammar MTGCostsParser;

options { tokenVocab=MTGLexer; }

// Costs rules
costList
    : cost (COMMA cost)*
    | cost AND cost
    ;

cost
    : manaCost
    | tapCost
    | untapCost
    | lifeCost
    | sacrificeCost
    | discardCost
    ;

manaCost
    : (MANA_W | MANA_U | MANA_B | MANA_R | MANA_G | MANA_C | MANA_X | GENERIC_MANA)+
    ;

tapCost
    : TAP_SYMBOL
    ;

untapCost
    : UNTAP_SYMBOL
    ;

lifeCost
    : PAY NUMBER LIFE
    ;

sacrificeCost
    : SACRIFICE targetSelector
    ;

discardCost
    : DISCARD cardFilter
    | DISCARD A CARD
    ;
