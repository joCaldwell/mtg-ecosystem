parser grammar MTGEffectsParser;

options { tokenVocab=MTGLexer; }

// Verbs and Spell effects
effect
    : primaryEffect (FOR targetSelector)?
    ;

primaryEffect
    : dealDamage
    | drawCards
    | destroyPermanent
    | createToken
    | modifyPT
    | counterSpell
    | returnEffect
    | conditionalEffect
    | exileEffect
    | gainLifeEffect
    | gainKeywordEffect
    | becomeEffect
    | gainControlEffect
    | tapEffect
    | untapEffect
    | putCounterEffect
    | putZoneEffect
    | searchEffect
    | shuffleEffect
    | addManaEffect
    | chooseEffect
    | revealEffect
    | sacrificeEffect
    | copyEffect
    | chooseNewTargets
    ;

exileEffect
    : EXILE targetSelector (locationSpec)?
    ;

gainLifeEffect
    : targetSelector GAINS valueExpression LIFE
    ;

gainKeywordEffect
    : targetSelector GAINS gainableAbilityList duration?
    ;

becomeEffect
    : targetSelector BECOMES (A)? (COPY OF targetSelector | cardFilter) (WITH (BASE)? POWER AND TOUGHNESS NUMBER SLASH NUMBER)? (duration)? (inAddition)?
    ;

inAddition
    : IN ADDITION TO ITS OTHER? (nameWord | cardType | colorQualifier)*
    ;

gainControlEffect
    : GAINS CONTROL OF targetSelector (duration)?
    ;

dealDamage
    : targetSelector DEALS valueExpression DAMAGE TO targetSelector
    ;

drawCards
    : DRAW valueExpression CARD
    | DRAW A CARD
    ;

destroyPermanent
    : DESTROY targetSelector
    ;

basePT
    : (NUMBER | X_VAR) SLASH (NUMBER | X_VAR)
    ;

tokenProperties
    : basePT? colorQualifier* nameWord* cardType*
    ;

createToken
    : CREATE (A | valueOrWord)? tokenProperties TOKEN (WITH keywordAbility ((AND | COMMA) keywordAbility)*)?
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

returnEffect
    : RETURN targetSelector TO possessivePronoun OWNERS HAND
    | RETURN targetSelector TO THE BATTLEFIELD (UNDER (controllerQualifier | possessivePronoun) OWNERS? CONTROL)?
    ;

conditionalEffect
    : IF condition COMMA? effectList (PERIOD OTHERWISE COMMA? effectList)?
    ;

tapEffect
    : TAP targetSelector
    ;

untapEffect
    : UNTAP targetSelector
    ;

putCounterEffect
    : PUT (A | valueExpression)? (ptModifier | nameWord+) COUNTER (ON targetSelector)?
    ;

putZoneEffect
    : PUT targetSelector locationSpec (TAPPED)?
    ;

searchEffect
    : SEARCH (controllerQualifier)? zone FOR cardFilter (COMMA? (putZoneEffect | returnEffect))* (COMMA? (THEN | AND)? SHUFFLE (controllerQualifier)? zone?)?
    ;

shuffleEffect
    : SHUFFLE (controllerQualifier)? zone?
    ;

manaSymbol
    : MANA_W | MANA_U | MANA_B | MANA_R | MANA_G | MANA_C | MANA_X | GENERIC_MANA | MANA_E
    ;

addManaEffect
    : ADD manaSymbol+
    | ADD manaSymbol (OR manaSymbol)*
    | ADD (A | valueOrWord) MANA OF ANY COLOR
    ;

chooseEffect
    : CHOOSE A? (CREATURE TYPE | CARD NAME | COLOR | cardFilter)
    ;

quotedAbility
    : DOUBLE_QUOTE ability DOUBLE_QUOTE
    ;

gainableAbility
    : keywordAbility
    | quotedAbility
    ;

gainableAbilityList
    : gainableAbility ((COMMA (AND)? | AND)? gainableAbility)*
    ;

revealEffect
    : (YOU MAY)? REVEAL cardFilter (locationSpec)?
    ;

sacrificeEffect
    : SACRIFICE targetSelector
    ;

copyEffect
    : (YOU MAY)? COPY targetSelector
    ;

chooseNewTargets
    : (YOU MAY)? CHOOSE NEW TARGETS (FOR THE COPY)?
    ;
