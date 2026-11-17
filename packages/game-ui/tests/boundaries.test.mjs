import assert from 'node:assert/strict';
import test from 'node:test';
import { presentationViolations } from '../scripts/check-boundaries.mjs';

test('presentation rejects protocol, private applications, routing and I/O', () => {
  for (const source of [
    'import { GameClient } from "@multiverse/game-server-sdk";',
    'import type { Observation } from "../../game/private.ts";',
    'export { Link } from "react-router";',
    'fetch("/api");', 'window.localStorage.getItem("x");',
    'const dependency = import("private");',
  ]) assert.ok(presentationViolations('view.tsx', source).length, source);
});
test('presentation allows React, own display models, and scoped styles', () => {
  assert.deepEqual(presentationViolations('view.tsx', 'import { useState } from "react"; import type { CardView } from "./table-model.ts"; import styles from "./view.module.css";'), []);
  assert.deepEqual(presentationViolations('view.module.css', '.surface { --game-accent: blue; color: var(--game-accent); }'), []);
});
test('shared CSS cannot change document styles or use host variable names', () => {
  for (const css of [':root { color: blue; }', 'body { margin: 0; }', '.x { color: var(--text); }']) {
    assert.ok(presentationViolations('view.module.css', css).length, css);
  }
});
