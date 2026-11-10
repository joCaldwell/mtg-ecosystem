# Optional controller draws

IR27 adds `OptionalControllerDrawV1` and the distinct
`OptionalDrawCards { source_block }` instruction. It means an optional single
card draw by the invocation's controller. Existing mandatory `DrawCards` data
and historical catalog bytes remain unchanged. Earlier IR rejects the new
opcode independently of its capability label, including in modal child programs.

The complete source witness is **Neurok Commando**, Oracle ID
`48936751-429b-4c40-88f5-72acde6278cc`, retained in
`packages/game-engine/tests/fixtures/cards-data-d2ce/`. It has shroud and
“Whenever this creature deals combat damage to a player, you may draw a card.”
Archive line 10885 has source SHA-256
`e4b72df3d9b9d520adb40480b3954b31b219d97c486c0861d74c7a6bce60de30`.
The compiler lowers the whole source, preserving both shroud and the trigger.
Its historical unsupported optional-draw disposition is superseded by this
explicit capability; the historical report retains its original evidence scope.

The trigger collects and goes on the stack normally. Its controller chooses
whether to draw during resolution, through the existing public `OptionalEffect`
request/response. The request exposes no library rows or hidden object handles.
Accepting uses ordinary draw movement, failed-draw and Commander hand-replacement
paths. Declining finishes that instruction without attempting a draw. An empty
library does not remove the option to accept. A departed controller's personal
draw is skipped; a surviving controller's invocation remains independent of
its source. Restoring an accepted optional draw cannot ask the option twice or
consume a second library card after Commander replacement.

Only one-card controller draws are admitted by this opcode. Optional target or
event-player draws, quantities other than one, and qualified actors reject the
complete source. No new public request, host/search admission or control-changing
mechanic is introduced. Trusted recovery remains bound to the exact build and
catalog and proves affirmative choice through accepted-input replay.

Rules authority is the retained 2026-09-25 Comprehensive Rules, SHA-256
`8d860e451f20f38865b725b42d82feb714c725373dd8f3b32b8652b3eeb070ca`:
CR 603.5 places optional triggers normally and defers the choice to resolution;
608.2d permits choosing to draw from an empty library; 109.5 identifies “you”;
113.7a preserves independent invocations; 121.3/704.5b define failed draws;
903.9b defines Commander hand replacement. Official source:
<https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt>.
