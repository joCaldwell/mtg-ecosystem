import assert from 'node:assert/strict';
import test from 'node:test';
import { boundaryViolations } from '../scripts/boundaries.ts';

test('boundaries reject IO, SDK runtime, private packages and layer inversions', () => {
  const invalid: [string, string][] = [
    ['features/a.tsx', 'fetch("/api")'],
    ['features/a.tsx', 'globalThis["fetch"]("/api")'],
    ['presentation/a.ts', 'import { useState } from "react"'],
    ['decisions/a.ts', 'window.localStorage.getItem("x")'],
    ['session/generation.ts', 'navigator.locks.request("seat", () => {})'],
    ['ui/a.tsx', 'import type { CardView } from "../presentation/table-model.ts"'],
    ['app/a.ts', 'import policy from "../../../game-server/sdk/src/examples/foundations-consumer.ts"'],
    ['app/a.ts', 'import policy from "../../../app/policy.ts"'],
    ['features/a.ts', 'import { GameClient } from "@multiverse/game-server-sdk"'],
    ['session/effects/a.ts', 'import { policy } from "@multiverse/game-server-sdk/src/examples/policy"'],
    ['presentation/a.ts', 'export { GameClient } from "@multiverse/game-server-sdk"'],
    ['app/a.ts', 'import("remote")'],
  ];
  for (const [file, source] of invalid) assert.ok(boundaryViolations(file, source).length, source);
});
test('pure generated types and correctly owned effects are allowed', () => {
  assert.deepEqual(boundaryViolations('presentation/a.ts', 'import type { Observation } from "@multiverse/game-server-sdk"'), []);
  assert.deepEqual(boundaryViolations('session/effects/a.ts', 'import { GameClient } from "@multiverse/game-server-sdk"; fetch("/api")'), []);
  assert.deepEqual(boundaryViolations('presentation/a.ts', 'import type { PresentationArtifact } from "../../generated/presentation.d.ts"'), []);
});
