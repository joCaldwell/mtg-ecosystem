---
name: commander-deck-building
description: Method for working on Magic: The Gathering Commander decks through the deck-builder MCP server — evaluating a deck against its brief, building a deck from a commander, reworking a fork, recommending what to build next, and tidying memories. Use when the deck-builder tools are available and the user asks about their decks, what to add or cut, a new deck, or their tastes as a builder.
---

# Commander deck building with the deck-builder app

The app's own playbook (returned by `start_session`) holds the rules: what you
may change, how cards are named, what to remember. This skill is method — how
to do the work well once the rules are followed. Where the two disagree, the
playbook wins.

## Every session

1. Call `start_session`. Read the profile and global memories before anything else.
2. If it reports unread rulings, read them (`list_rulings` with `unreviewed_only`), write or revise memories for the ones that say something lasting, then `mark_rulings_reviewed`. Do this before proposing: a reason the owner gave yesterday may answer today's question.
3. Work out which deck is meant. "This deck" is the focus. If nothing is in focus and the request is about a deck, ask or use the roster.

## Evaluating a deck

Read it with `get_deck`. Then answer these in order, from the oracle text in
front of you, not from memory of the cards:

1. **Does the list do what the brief says?** State the thesis in one sentence, then name the cards that carry it. If you cannot find enough, that is the finding. If the brief is empty, stop and help the owner write a thesis first.
2. **How does it win?** Trace one line from an ordinary opening to a won game. Name the cards. If the line needs a specific card with no redundancy, say so.
3. **Do the engines hold?** For each engine in the brief, the checks already say whether a piece is missing. Your part is redundancy: what happens when the key piece is removed.
4. **Can it get there?** Read the computed state: lands, curve, slot deltas. Compare ramp and card advantage with what the plan costs. Do not recount; quote the app's numbers.
5. **Can it survive and interact?** Count is the app's; judgement about whether the interaction answers what this owner's table plays is yours, and their profile and memories tell you about that table.
6. **What works against the deck?** Look for cards that undo an engine or hand symmetry back.

Report the two or three findings that matter most. Then propose changes for
those, not a general tune-up.

## Proposing changes

- Size the proposal to the request. "Fix the mana" is the mana. "Rebuild around this" on a Paper deck is a reason to suggest forking, not a sixty-item proposal.
- Search in the deck's context (`search_cards` with `deck`) so results respect its colour identity and arrive with the owner's prior rulings on each card.
- For each add, decide what it replaces. A deck at 100 needs a cut for every add; group the pair when the argument for one depends on the other.
- Write each reason so it stands alone when read in the app, away from this conversation: what the card does here, quoting the clause, and what it is better than.
- Check the result of `propose_changes`. It shows what the app already knows about those cards. If a prior ruling contradicts your reason, say so to the owner at once.

## Comparing sources and keeping proposals current

Use `compare_deck_sources` with the target and every source deck together.
Read its `shared_cards`, `remaining_cards`, evidence, unknown quantities and
application-computed totals. Use `apply_source_tags` with all observed revisions
when the owner asks to label sources. Treat source membership as a reuse
candidate; it does not prove a physical copy is free. `Have` records ownership,
`Keep` records preference, and unknown ownership remains unresolved. Record
new evidence with `record_ownership` only when the owner has supplied it.

When a package leaves the list, inspect engine findings and pending proposals.
Propose related brief repairs in one `propose_document_changes` group. If the
budget or finishing plan changes again, amend the whole group with
`revise_proposal` or explicitly name it in `supersedes`; carry forward every
still-needed change. Withdraw abandoned drafts. Read `list_proposals` to verify
one current plan remains, with earlier drafts linked in history. Approval of
brief and power targets remains the owner's action.

## Building from a commander

1. Look the commander up (`get_cards`). Read what it actually does.
2. Agree a thesis with the owner in a sentence or two, and their constraints: power level, budget, anything the profile already answers.
3. `create_deck` with the commander and the thesis. It is Theorycrafted: you edit its cards directly; the supplied thesis is a proposal.
4. `define_slots` with targets that fit this plan. Typical starting points for a 100-card deck are roughly 36–38 lands, 10–12 ramp, 10 card advantage, 10 interaction, with the rest carrying the thesis; adjust to the curve and say why.
   File the principal purpose as `slot`, and additional purposes as `secondary_slots`. Targets count primary assignments only; read the app's separate secondary coverage before deciding a role is missing. Tags track where cards come from, ownership, assembly and status, rather than card purposes.
5. Fill slot by slot with `edit_cards`, searching per slot. After each batch read the returned counts and checks, and steer by them.
6. When the Theorycrafted deck is at 100 with no check problems, walk the owner through it by slot. Record engines with `propose_brief_edit`. Only the owner changes lifecycle to Paper. Brief edits always require approval; Legacy decks are read-only.

## Reworking a deck the owner wants to keep

Fork it (`fork_deck`). The fork is Theorycrafted with the parent's list, brief and
memories. Change the fork's thesis first, then the cards. `compare_decks` on
the fork shows what has changed from the parent; use it to summarise.

## What should I build next

1. `player_stats` for what they build: colours, commander types, curve, the cards they reuse.
2. The profile and global memories for what they enjoy and avoid.
3. Offer two or three directions that differ from each other: one that deepens something they clearly like, one that fills a gap (a colour or play pattern absent from the stats), each with a commander you have looked up and read.
4. Say which you would pick and why. Offer to draft the one they choose.

Do not recommend a commander you have not read with `get_cards` in this
conversation.

## Tidying memories

When a scope is over budget, or the owner asks:

1. Read the whole scope with `search_memories` (include retired only if history matters).
2. Merge memories that say the same thing: supersede one with the combined text, retire the others.
3. Supersede what a later ruling has changed.
4. Promote to global what has now shown up in several decks.
5. Where a cluster has settled into something durable, propose folding it into the profile or the brief, and retire the memories once the owner accepts.
6. Leave the owner's own memories alone. If one looks out of date, say so.

Tell the owner what you merged and retired.
