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
    | conditionalEffect
    ;

dealDamage
    : selfQualifier DEALS valueExpression DAMAGE TO targetSelector
    ;

drawCards
    : DRAW valueExpression CARD
    | DRAW A CARD
    ;

destroyPermanent
    : DESTROY targetSelector
    ;

createToken
    : CREATE valueExpression IDENTIFIER* TOKEN
    ;

modifyPT
    : targetSelector GETS ptModifier duration
    ;

ptModifier
    : PLUS valueExpression SLASH PLUS valueExpression
    | MINUS valueExpression SLASH MINUS valueExpression
    | PLUS valueExpression SLASH MINUS valueExpression
    | MINUS valueExpression SLASH PLUS valueExpression
    ;

counterSpell
    : COUNTER targetSelector
    ;

returnToHand
    : RETURN targetSelector TO ITS OWNERS HAND
    ;

conditionalEffect
    : IF condition COMMA? effectList (PERIOD OTHERWISE COMMA? effectList)?
    ;
