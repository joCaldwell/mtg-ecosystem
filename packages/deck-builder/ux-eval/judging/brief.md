# Product brief: the deck builder

You are looking at screenshots of a web app for building and maintaining
Magic: The Gathering **Commander** decks. Read this brief before you judge.

## Who uses it

- **Commander players** who build and maintain decks over long sessions:
  reading card text, comparing options, tracking which cards they own or still
  need, and keeping a deck's plan written down. Many have several decks at
  once, some played on paper, some still theory. They use it on a laptop or
  desktop most of the time, on a tablet sometimes, and on a phone mostly to
  read and to make quick decisions.
- **External AI agents** (for example Claude Code or Codex) that the owner
  connects to the app. Agents read the decks and **propose** changes: add or
  cut a card, edit a deck's brief, remember a preference. The app makes no AI
  calls of its own. The owner reviews each proposal and accepts or rejects it,
  optionally with a reason the agent reads next time. Everything an agent
  wrote is attributed to it.

Other people can see a deck only if the owner shares it: an unlisted link or
a public page, which anyone can open without an account.

## The core tasks

1. Start: create a deck, import a list from another site (Archidekt,
   Moxfield), or try the sample deck.
2. Edit a deck: find and add cards, cut cards, keep a maybe list of
   candidates, sort cards into purposes ("slots", with targets such as
   "10–12 ramp") and tags ("Need to buy", "Testing").
3. Keep the deck's plan: a brief (thesis and constraints), power target,
   checks (card count, colour identity, legality).
4. Review agent proposals: see what would change and why, then accept or
   reject each one, with an optional reason. On a phone too.
5. Look back: the history of decisions, the activity feed of agent writes,
   memories agents keep about the owner and each deck.
6. Share: make a deck public or unlisted and send the link; view a public
   deck or profile without signing in.

## Lifecycle words you will see

- **Paper**: a deck the owner plays. Agents may only propose card changes.
- **Theorycrafted**: a work in progress. Agents may edit cards directly.
- **Archived**: read-only.

## Design principles the app commits to

1. **The deck list is home.** Everything else comes to it: a side panel, a
   banner, a hover card. Nothing covers the list except a dialog the owner
   asked for.
2. **Colour means something or it is grey.** Violet: an agent wrote or
   proposed it. Green: coming in, passing, accepted. Red: going out, failing,
   rejected. Amber: off target or missing. Blue: the owner's selection and
   focus. Everything else is grey. Mana colours appear in mana symbols and in the
   paired colour-distribution charts in deck Stats. The charts compare fixed
   coloured cost symbols and recognized source-colour counts, each normalized
   to its own total; multicolour sources contribute to multiple colour counts.
3. **Text first.** Cards are names, costs and rules text; the card picture
   appears on demand. Public deck pages show the commander's art.
4. **Dense, not cramped.** Small body text and one line per card on desktop,
   because owners scan a hundred cards at once. Space goes between groups,
   not between rows. On touch screens, controls must still be easy to tap.
5. **Every agent write is visibly an agent's**, with the agent's name.
6. **Reasons are cheap**: any decision can take a short optional reason.
7. **Numbers come from the app** and are shown in a monospaced face.

Phones (under 720 px) are for reading and deciding; screens that cannot work
there say "Open on a larger screen". From 720 px up it is the full app.

## Desktop interface direction

Deck-level panel destinations remain visible together on one line. Card
inspection replaces that navigation with Back and the card header; Back
returns to the previous deck view. Stats and Checks are separate destinations.
Page titles and actions share the content frame, without a full-width header
band or explanatory subtitle. Keep help specific and concise; ordinary
controls should explain themselves. Maybe rows open card details across the
row and do not display purpose assignments.

## My decks

My decks is the owner's collection, and it is the one place where style comes
before density. It should feel like a showcase of the decks' commander art
rather than a table. Judge its density by whether the page feels composed, not
by how many decks fit on a screen; principles 3 and 4 above are about the deck
editor, not this page. The unboxed artist credit is intentionally barely visible
at rest, and can become readable on tile hover or keyboard focus.

## Deck header

The deck page opens with a large commander-art header that yields its height
once the owner is working in the list. Principle 3 (text first) applies to the
card list, rather than the header. Artist credits are unboxed and deliberately
very low emphasis at rest.

## How to read the screenshots

Each image is listed in `index.md` with its viewport width, the state it
shows and a short caption. Some images are a task shown as numbered steps,
each step captioned with the action just taken. Card data, names and prices
are realistic fixtures; dates are fixed. Judge the interface, not the
example content.
