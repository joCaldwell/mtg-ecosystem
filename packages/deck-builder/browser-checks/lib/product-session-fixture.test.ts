import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { completeResponse, verifyInputs, startHostedFixture } from "./product-session-fixture.ts";

test("acceptance compares complete responses including FinishMana reversal and rejects frozen qualification before native I/O", () => {
  completeResponse({ response: "finish_mana", undo_mana: true }, { response: "finish_mana", undo_mana: true });
  assert.throws(() => completeResponse({ response: "finish_mana", undo_mana: false }, { response: "finish_mana", undo_mana: true }), /complete_response/);
  assert.throws(() => completeResponse({ response: "activate_ability", object: "fixture", ability: 3 }, { response: "activate_ability", object: "fixture", ability: 2 }), /complete_response/);
  const directory = mkdtempSync(join(tmpdir(), "product-frozen-qualification-"));
  const qualification = join(directory, "pins.json");
  writeFileSync(qualification, JSON.stringify({ native_source: "565a1204d6e4b14443b9f49dda2ecbc6b46ee85a", sdk_source: "565a1204d6e4b14443b9f49dda2ecbc6b46ee85a" }), { mode: 0o600 });
  assert.throws(() => verifyInputs({ qualification,
    nativeRoot: "/must-not-run-native", sdkRoot: "/unused", records: "/unused", recordsHash: "" }), /new_qualified_source_required/);
});

test("disposable public hosted factory verifies real signed BetterAuth cookies and principal isolation", async () => {
  process.umask(0o077);
  const parent = mkdtempSync(join(tmpdir(), "product-session-auth-"));
  const fixture = await startHostedFixture({ directory: join(parent, "private") });
  try {
    const call = (path: string, cookie?: string) => fetch(fixture.origin + path, { headers: cookie ? { Cookie: `better-auth.session_token=${cookie}` } : {}, signal: AbortSignal.timeout(5_000) });
    for (const principal of [fixture.owner, fixture.foreign]) {
      const response = await call("/api/session", principal.cookie);
      assert.equal(response.status, 200);
      const session = await response.json() as { user: { id: number } };
      assert.equal(session.user.id === principal.user_id, true);
    }
    assert.equal(fixture.owner.user_id !== fixture.foreign.user_id && fixture.owner.cookie !== fixture.foreign.cookie, true);
    for (const cookie of [undefined, `${fixture.owner.cookie}invalid`]) {
      assert.equal((await call("/api/decks", cookie)).status, 401);
      const session = await (await call("/api/session", cookie)).json() as { user: unknown };
      assert.equal(session.user, null);
    }
  } finally { await fixture.close(); }
});
