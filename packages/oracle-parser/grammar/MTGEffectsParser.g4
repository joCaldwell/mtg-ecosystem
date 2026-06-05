parser grammar MTGEffectsParser;

options { tokenVocab=MTGLexer; }

// Verbs and Spell effects
effect
    : dealDamage
    | drawCards
    | destroyPermanent
    | createToken
    | modifyPT
    | counterSpell
    | returnToHand
    ;

dealDamage
    : selfQualifier DEALS NUMBER DAMAGE TO targetSelector
    ;

drawCards
    : DRAW NUMBER CARD
    | DRAW A CARD
    ;

destroyPermanent
    : DESTROY targetSelector
    ;

createToken
    : CREATE NUMBER IDENTIFIER* TOKEN
    ;

modifyPT
    : targetSelector GETS ptModifier duration
    ;

ptModifier
    : PLUS NUMBER SLASH PLUS NUMBER
    | MINUS NUMBER SLASH MINUS NUMBER
    | PLUS NUMBER SLASH MINUS NUMBER
    | MINUS NUMBER SLASH PLUS NUMBER
    ;

counterSpell
    : COUNTER targetSelector
    ;

returnToHand
    : RETURN targetSelector TO ITS OWNERS HAND
    ;
