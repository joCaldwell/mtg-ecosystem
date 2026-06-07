parser grammar MTGModifiersParser;

options { tokenVocab=MTGLexer; }

// Modifiers and Conditions
duration
    : UNTIL END OF TURN
    | THIS TURN
    | (FOR)? AS LONG AS condition
    | DURING controllerQualifier TURN
    | DURING EACH PLAYER TURN
    ;

condition
    : controllerQualifier CONTROL targetSelector
    | targetSelector PAYS cost
    | controllerQualifier (DO | DONT)
    | (IT | targetSelector) IS statusQualifier
    | (IT | targetSelector) IS (A)? cardFilter
    | targetSelector HAVE valueExpression (OR LESS | OR MORE_WORD)? LIFE
    | IT IS controllerQualifier TURN
    | IT REMAINS locationSpec
    ;
