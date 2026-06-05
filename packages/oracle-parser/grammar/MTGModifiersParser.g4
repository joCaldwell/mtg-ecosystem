parser grammar MTGModifiersParser;

options { tokenVocab=MTGLexer; }

// Modifiers and Conditions
duration
    : UNTIL END OF TURN
    | THIS TURN
    | FOR AS LONG AS YOU CONTROL selfQualifier
    ;

condition
    : IF controllerQualifier CONTROL cardFilter
    | UNLESS targetSelector PAYS cost
    ;
