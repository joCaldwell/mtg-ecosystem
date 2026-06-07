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
    | exileCost
    | returnCost
    | loyaltyCost
    ;

exileCost
    : EXILE targetSelector (locationSpec)?
    ;

manaCost
    : (MANA_W | MANA_U | MANA_B | MANA_R | MANA_G | MANA_C | MANA_X | GENERIC_MANA)+
    ;

tapCost
    : TAP_SYMBOL
    | TAP targetSelector
    ;

untapCost
    : UNTAP_SYMBOL
    | UNTAP targetSelector
    ;

lifeCost
    : PAY valueExpression LIFE
    ;

sacrificeCost
    : SACRIFICE targetSelector
    ;

discardCost
    : DISCARD cardFilter
    | DISCARD A CARD
    ;

returnCost
    : RETURN targetSelector TO possessivePronoun OWNERS HAND
    ;

loyaltyCost
    : (PLUS | MINUS) (NUMBER | X_VAR)
    | NUMBER
    ;
