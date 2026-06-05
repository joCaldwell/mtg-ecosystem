parser grammar MTGValuesParser;

options { tokenVocab=MTGLexer; }

valueExpression
    : mathExpression
    | simpleValue
    ;

simpleValue
    : NUMBER
    | X_VAR
    | attributeValue
    | variableValue
    ;

attributeValue
    : ITS POWER
    | ITS TOUGHNESS
    ;

variableValue
    : THE NUMBER_WORD OF cardFilter
    | THE NUMBER_WORD OF cardFilter locationSpec
    ;

mathExpression
    : simpleValue PLUS_WORD simpleValue
    | TWICE simpleValue
    | HALF simpleValue (ROUNDED (UP | DOWN))?
    ;
