# Game Server (Layer 2)

This directory is planned for the Rust game hosting and API server.
It links the Rust rules core directly and hosts Commander pods, sessions,
persistence, clocks, and remote-agent coordination.

The language decision is adopted; no server or API is implemented yet.
Choose the Rust framework and network transport at
[milestone P8](../../docs/game-engine-plan.md#p8--server-and-external-consumers).
Browser and application clients use the generated TypeScript protocol contract.
See the [engine protocol boundary](../../docs/game-engine-spec.md#5-core-execution-and-decisions)
and [adopted language decision](../../docs/runtime-language-decision.md).
