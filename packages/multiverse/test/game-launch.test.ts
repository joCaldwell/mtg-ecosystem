import test from "node:test";
import assert from "node:assert/strict";
import { admissionMessage } from "../web/src/domain/gameLaunch.ts";
import type { GameAdmission } from "../web/src/api/client.ts";

test("launch states distinguish stale/unsupported/unavailable and admitted but incomplete sessions", () => {
  const cases: Array<[GameAdmission, RegExp]> = [
    [{ state: "not_checked" }, /not been checked/],
    [{ state: "checking" }, /Checking/],
    [{ state: "stale_revision", current_revision: 9 }, /revision 9/],
    [{ state: "unsupported", code: "multiple_commanders" }, /multiple commanders/],
    [{ state: "unsupported", code: "companion" }, /companions/],
    [{ state: "unavailable", reason: "native_unavailable" }, /unavailable/],
    [{ state: "unavailable", reason: "identity_mismatch" }, /does not match/],
    [{ state: "supported", identity: { catalog_sha256: "b".repeat(64), profile: "fixture", engine_build_id: "engine", server_build_id: "host" }, deck_sha256: "c".repeat(64), launch_ready: false }, /not connected yet/],
  ];
  for (const [admission, expected] of cases) assert.match(admissionMessage(admission), expected);
});
