lexer grammar MTGLexer;

// Syntax markers
COLON : ':' ;
COMMA : ',' ;
PERIOD : '.' ;
TILDE : '~' ;
PLUS : '+' ;
MINUS : '-' ;
SLASH : '/' ;

// Mana Symbols
MANA_W : '{W}' ;
MANA_U : '{U}' ;
MANA_B : '{B}' ;
MANA_R : '{R}' ;
MANA_G : '{G}' ;
MANA_C : '{C}' ;
MANA_X : '{X}' ;
TAP_SYMBOL : '{T}' ;
UNTAP_SYMBOL : '{Q}' ;
GENERIC_MANA : '{' [0-9]+ '}' ;

// Verbs, Nouns and Keywords (Case-insensitive where needed)
AND : 'and' ;
PAY : 'Pay' | 'pay' ;
LIFE : 'life' ;
SACRIFICE : 'Sacrifice' | 'sacrifice' ;
DISCARD : 'Discard' | 'discard' ;
A : 'a' | 'A' ;
CARD : 'card' | 'cards' | 'Card' | 'Cards' ;
TARGET : 'target' | 'Target' ;
ANY : 'any' | 'Any' ;
EACH : 'each' | 'Each' ;
ALL : 'all' | 'All' ;
YOU : 'you' | 'You' ;
YOUR : 'your' | 'Your' ;
OPPONENT : 'opponent' | 'Opponent' | 'opponents' | 'Opponents' ;
CREATURE : 'creature' | 'creatures' | 'Creature' | 'Creatures' ;
PLAYER : 'player' | 'Player' | 'players' | 'Players' | 'player\'s' | 'Player\'s' ;
PLANESWALKER : 'planeswalker' | 'Planeswalker' | 'planeswalkers' | 'Planeswalkers' ;
PERMANENT : 'permanent' | 'Permanent' | 'permanents' | 'Permanents' ;
SPELL : 'spell' | 'Spell' | 'spells' | 'Spells' ;
LAND : 'land' | 'Land' | 'lands' | 'Lands' ;
ARTIFACT : 'artifact' | 'Artifact' | 'artifacts' | 'Artifacts' ;
ENCHANTMENT : 'enchantment' | 'Enchantment' | 'enchantments' | 'Enchantments' ;

BATTLEFIELD : 'battlefield' | 'Battlefield' ;
GRAVEYARD : 'graveyard' | 'Graveyard' ;
LIBRARY : 'library' | 'Library' ;
HAND : 'hand' | 'Hand' ;
EXILE : 'exile' | 'Exile' ;

UNTIL : 'until' | 'Until' ;
END : 'end' | 'End' ;
OF : 'of' | 'Of' ;
TURN : 'turn' | 'Turn' ;
THIS : 'this' | 'This' ;

FOR : 'for' | 'For' ;
AS : 'as' | 'As' ;
LONG : 'long' | 'Long' ;
CONTROL : 'control' | 'Control' ;

IF : 'if' | 'If' ;
UNLESS : 'unless' | 'Unless' ;
PAYS : 'pays' | 'Pays' ;

WHEN : 'When' | 'when' ;
WHENEVER : 'Whenever' | 'whenever' ;
AT : 'At' | 'at' ;
BEGINNING : 'beginning' | 'Beginning' ;
BEGINS : 'begins' | 'Begins' ;
UPKEEP : 'upkeep' | 'Upkeep' ;
CASTS : 'casts' | 'Casts' ;
CAST : 'cast' | 'Cast' ;
DIES : 'dies' | 'Dies' ;
ENTERS : 'enters' | 'Enters' ;
THE : 'the' | 'The' ;

DEALS : 'deals' | 'Deals' ;
DAMAGE : 'damage' | 'Damage' ;
TO : 'to' | 'To' ;
DRAW : 'draw' | 'Draw' ;
DESTROY : 'destroy' | 'Destroy' ;
CREATE : 'create' | 'Create' ;
TOKEN : 'token' | 'tokens' | 'Token' | 'Tokens' ;
GETS : 'gets' | 'Gets' ;
GET : 'get' | 'Get' ;
HAVE : 'have' | 'Have' ;
COUNTER : 'counter' | 'Counter' ;
RETURN : 'return' | 'Return' ;
ITS : 'its' | 'Its' ;
OWNERS : 'owner\'s' | 'Owner\'s' ;

// Keywords
FLYING : 'Flying' | 'flying' ;
VIGILANCE : 'Vigilance' | 'vigilance' ;
HASTE : 'Haste' | 'haste' ;
TRAMPLE : 'Trample' | 'trample' ;
DEATHTOUCH : 'Deathtouch' | 'deathtouch' ;
LIFELINK : 'Lifelink' | 'lifelink' ;
FIRST : 'First' | 'first' ;
STRIKE : 'strike' | 'Strike' ;
DOUBLE : 'Double' | 'double' ;
DEFENDER : 'Defender' | 'defender' ;
REACH : 'Reach' | 'reach' ;

WARD : 'Ward' | 'ward' ;
EQUIP : 'Equip' | 'equip' ;
KICKER : 'Kicker' | 'kicker' ;
EVOKE : 'Evoke' | 'evoke' ;

// Basic values
NUMBER : [0-9]+ ;
IDENTIFIER : [a-zA-Z]+ ;

// Whitespace
WS : [ \t\r\n]+ -> skip ;
