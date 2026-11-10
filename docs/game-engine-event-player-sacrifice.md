# Creature sacrifice by a frozen damage recipient

IR29 adds `EventPlayerSacrificeV1` and
`SacrificeCreature { recipient, source_block }`, meaning one creature sacrificed
by the frozen damage recipient. It shares the `ActionRecipient` vocabulary with
discard. That internal type replaces the earlier `DiscardRecipient` name without
an alias or changes to serialized `controller`/`event_player` values. Existing
discard instructions and catalog bytes retain their meaning.

The complete source witness is **Demon of Loathing**, Oracle ID
`2e55fc03-779c-4a71-beb7-aa0ade738c24`, retained in
`packages/game-engine/tests/fixtures/cards-data-d2ce/`. Its full source has flying,
trample and “Whenever this creature deals combat damage to a player, that player
sacrifices a creature of their choice.” Archive line 6875 has source SHA-256
`6c2c3d5f202fd7be15ac0830cf4cc3d57809d1d59c8638a9aafde497219f085f`.
The compiler consumes both keyword and trigger blocks. Its former unsupported
disposition is superseded without changing the retained record or historical
report's evidence scope.

The event player chooses through the existing `Sacrifice` request, from creatures
they control. Ownership does not determine eligibility: sacrificing a borrowed
creature moves it to its owner's graveyard. This is not destruction. Source
control, source departure and later combat state do not change the frozen actor.
Empty creature domains and departed actors skip the action. Started supported
resolution remains independent of the source and invocation controller's later
departure. Ordinary sacrifice events collect death triggers and schedule
Commander graveyard returns at the normal post-program stabilization boundary.

Only an event-player recipient bound by a damage trigger is admitted for the new
instruction. Controller sacrifice, filtered/noncreature sets, counts other than
one, qualified/mismatched chooser and actor, optional programs and unbound
spell/activation/step/cast contexts reject. New opcode/version gates apply
recursively, independently of capability labels. Existing draw/life event-player
bindings, public requests and hosted/search capability sets remain unchanged.

Rules authority is the retained 2026-09-25 Comprehensive Rules, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`:
CR 701.21a defines control, ownership and non-destruction for sacrifice;
603.3a/608.2c preserve invocation control and written order; 609.3 performs the
possible parts; 117.5/704.4 defer stabilization during resolution; 903.9a defines
Commander graveyard return. Official source:
<https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>.
