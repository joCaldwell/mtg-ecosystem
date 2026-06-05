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
A : 'a' | 'A' | 'an' | 'An' ;
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
FROM : 'from' | 'From' | 'FROM' ;
IN : 'in' | 'In' | 'IN' ;
ON : 'on' | 'On' | 'ON' ;
DRAW : 'draw' | 'Draw' ;
DESTROY : 'destroy' | 'Destroy' ;
CREATE : 'create' | 'Create' ;
TOKEN : 'token' | 'tokens' | 'Token' | 'Tokens' ;
GETS : 'gets' | 'Gets' ;
GET : 'get' | 'Get' ;
HAVE : 'have' | 'Have' | 'has' | 'Has' | 'HAVE' | 'HAS' ;
COUNTER : 'counter' | 'Counter' ;
RETURN : 'return' | 'Return' ;
ITS : 'its' | 'Its' ;
OWNERS : 'owner\'s' | 'Owner\'s' ;

POWER : 'power' | 'Power' | 'POWER' ;
TOUGHNESS : 'toughness' | 'Toughness' | 'TOUGHNESS' ;

X_VAR : 'X' | 'x' ;
TWICE : 'twice' | 'Twice' | 'TWICE' ;
HALF : 'half' | 'Half' | 'HALF' ;
ROUNDED : 'rounded' | 'Rounded' | 'ROUNDED' ;
UP : 'up' | 'Up' | 'UP' ;
DOWN : 'down' | 'Down' | 'DOWN' ;
PLUS_WORD : 'plus' | 'Plus' | 'PLUS' ;
EQUAL : 'equal' | 'Equal' | 'EQUAL' ;

TAPPED : 'tapped' | 'Tapped' | 'TAPPED' ;
UNTAPPED : 'untapped' | 'Untapped' | 'UNTAPPED' ;
ATTACKING : 'attacking' | 'Attacking' | 'ATTACKING' ;
BLOCKING : 'blocking' | 'Blocking' | 'BLOCKING' ;

WHITE : 'white' | 'White' | 'WHITE' ;
BLUE : 'blue' | 'Blue' | 'BLUE' ;
BLACK : 'black' | 'Black' | 'BLACK' ;
RED : 'red' | 'Red' | 'RED' ;
GREEN : 'green' | 'Green' | 'GREEN' ;
COLORLESS : 'colorless' | 'Colorless' | 'COLORLESS' ;
MULTICOLORED : 'multicolored' | 'Multicolored' | 'MULTICOLORED' ;

NON_COLOR
    : ('nongreen' | 'Nongreen' | 'non-green' | 'Non-green'
      |'nonblack' | 'Nonblack' | 'non-black' | 'Non-black'
      |'nonred' | 'Nonred' | 'non-red' | 'Non-red'
      |'nonwhite' | 'Nonwhite' | 'non-white' | 'Non-white'
      |'nonblue' | 'Nonblue' | 'non-blue' | 'Non-blue'
      |'noncolorless' | 'Noncolorless' | 'non-colorless' | 'Non-colorless')
    ;

LEGENDARY : 'legendary' | 'Legendary' | 'LEGENDARY' ;
BASIC : 'basic' | 'Basic' | 'BASIC' ;
SNOW : 'snow' | 'Snow' | 'SNOW' ;
WORLD : 'world' | 'World' | 'WORLD' ;

NONLAND : 'nonland' | 'Nonland' | 'non-land' | 'Non-land' ;
NONCREATURE : 'noncreature' | 'Noncreature' | 'non-creature' | 'Non-creature' ;
NONBASIC : 'nonbasic' | 'Nonbasic' | 'non-basic' | 'Non-basic' ;
NONTOKEN : 'nontoken' | 'Nontoken' | 'non-token' | 'Non-token' ;
NONARTIFACT : 'nonartifact' | 'Nonartifact' | 'non-artifact' | 'Non-artifact' ;
NONENCHANTMENT : 'nonenchantment' | 'Nonenchantment' | 'non-enchantment' | 'Non-enchantment' ;

NUMBER_WORD : 'number' | 'Number' | 'NUMBER' ;
ANOTHER : 'another' | 'Another' | 'ANOTHER' ;
OTHER : 'other' | 'Other' | 'OTHER' ;

CHOOSE : 'choose' | 'Choose' | 'CHOOSE' ;
ONE : 'one' | 'One' | 'ONE' ;
TWO : 'two' | 'Two' | 'TWO' ;
THREE : 'three' | 'Three' | 'THREE' ;
BOTH : 'both' | 'Both' | 'BOTH' ;
OR : 'or' | 'Or' | 'OR' ;
MORE_WORD : 'more' | 'More' | 'MORE' ;
EM_DASH : '—' | '--' ;
BULLET : '•' | '*' ;
DO : 'do' | 'Do' | 'DO' ;
IT : 'it' | 'It' | 'IT' ;
IS : 'is' | 'Is' | 'IS' ;
DURING : 'during' | 'During' | 'DURING' ;
REMAINS : 'remains' | 'Remains' | 'REMAINS' ;
OTHERWISE : 'otherwise' | 'Otherwise' | 'OTHERWISE' ;

THEN : 'then' | 'Then' | 'THEN' ;
BECOMES : 'becomes' | 'Becomes' | 'become' | 'Become' | 'BECOMES' | 'BECOME' ;
GAINS : 'gains' | 'Gains' | 'gain' | 'Gain' | 'GAINS' | 'GAIN' ;
WITH : 'with' | 'With' | 'WITH' ;
BASE : 'base' | 'Base' | 'BASE' ;
ADDITION : 'addition' | 'Addition' | 'ADDITION' ;
COPY : 'copy' | 'Copy' | 'copies' | 'Copies' | 'COPY' | 'COPIES' ;
UNDER : 'under' | 'Under' | 'UNDER' ;
CONTROLS : 'controls' | 'Controls' ;

GREATER : 'greater' | 'Greater' | 'GREATER' ;
LESS : 'less' | 'Less' | 'LESS' ;
THAN : 'than' | 'Than' | 'THAN' ;
MANA : 'mana' | 'Mana' | 'MANA' ;
VALUE : 'value' | 'Value' | 'value\'s' | 'Value\'s' ;
ARE : 'are' | 'Are' | 'ARE' ;

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
