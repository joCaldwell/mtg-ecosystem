import test from "node:test";
import assert from "node:assert/strict";
import { admissionMessage } from "@multiverse/game/admission";
import type { GameAdmission } from "../shared/contracts.ts";

test("launch states distinguish stale/unsupported/unavailable and admitted but incomplete sessions", () => {
  const cases: Array<[GameAdmission, RegExp]> = [
    [{ state: "not_checked" }, /Check compatibility to start/],
    [{ state: "checking" }, /Checking/],
    [{ state: "stale_revision", current_revision: 9 }, /Your deck changed/],
    [{ state: "unsupported", code: "multiple_commanders" }, /multiple commanders/],
    [{ state: "unsupported", code: "companion" }, /companions/],
    [{ state: "unavailable", reason: "native_unavailable" }, /unavailable/],
    [{ state: "unavailable", reason: "identity_mismatch" }, /unavailable/],
    [{ state: "supported", identity: { catalog_sha256: "b".repeat(64), profile: "fixture", engine_build_id: "engine", server_build_id: "host" }, deck_sha256: "c".repeat(64), launch_ready: false }, /Live games are unavailable/],
  ];
  for (const [admission, expected] of cases) assert.match(admissionMessage(admission), expected);
});
