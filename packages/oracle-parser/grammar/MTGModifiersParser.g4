parser grammar MTGModifiersParser;

options { tokenVocab=MTGLexer; }

// Modifiers and Conditions
duration
    : UNTIL END OF TURN
    | THIS TURN
    | FOR AS LONG AS condition
    | DURING controllerQualifier TURN
    | DURING EACH PLAYER TURN
    ;

condition
    : controllerQualifier CONTROL targetSelector
    | targetSelector PAYS cost
    | controllerQualifier DO
    | (IT | targetSelector) IS statusQualifier
    | IT IS controllerQualifier TURN
    | IT REMAINS locationSpec
    ;
