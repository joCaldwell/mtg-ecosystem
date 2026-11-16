import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { completeResponse, verifyInputs, startHostedFixture, repository, fileHash } from "./product-session-fixture.ts";

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
    assert.equal(fixture.uiRateBudget, 240); assert.equal(fixture.uiRateWindowMs, 60_000);
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

test("coqualified native/SDK inputs verify the compiled closure and reject stale source/build/receipt pins before native I/O", {
  skip: process.env.MULTIVERSE_TEST_BROWSER_QUALIFICATION !== "1",
}, () => {
  const qualification = join(repository, "packages/game-client/sdk-qualification.json");
  const pins = JSON.parse(readFileSync(qualification, "utf8"));
  const inputs = { qualification, nativeRoot: join(pins.evidence_root, "native"), sdkRoot: join(pins.sdk_evidence_root, "sdk"),
    // This gate hashes an opaque input; it does not ingest cards or start a host.
    records: qualification, recordsHash: fileHash(qualification) };
  assert.equal(verifyInputs(inputs).native_source, pins.native_source);
  const directory = mkdtempSync(join(tmpdir(), "product-coqualified-inputs-"));
  const modified = join(directory, "pins.json");
  for (const [patch, failure] of [
    [{ server_build_id: "c1be893414c34a3869f059a872b053d4c69e77da21e98242ffb654a980372d15" }, "native_build_pin"],
    [{ server_build_id: "0d29823bcbe03b17bd4d6aeab2765c2b7b8ea9d93abb7833e0b96fcfe4c61d82" }, "native_build_pin"],
    [{ native_source_manifest_sha256: "1eddba690527db4a1654f7556cf97c35d90225b2f7652141f640bc9b8842273a" }, "native_source_manifest_pin"],
    [{ native_source_manifest_sha256: "1915ae68e93ec55dbf4a1f6aa976d7d147bd662f0890a98ec32608609b9c8963" }, "native_source_manifest_pin"],
    [{ sdk_verification_sha256: "c0ea983fa64820d4b47f0c009ec8351fb832cc6ea9e8b8ce15535af1450872ac" }, "coqualified_sdk_receipt_pin"],
    [{ native_source_verification_sha256: "0".repeat(64) }, "coqualified_native_verification_pin"],
    [{ native_source: "f".repeat(40), sdk_source: "f".repeat(40) }, "coqualified_native_source_binding"],
    [{ sdk_source_bridge_sha256: undefined }, "coqualified_source_proof_required"],
    [{ sdk_evidence_root: join(pins.sdk_evidence_root, "../product-presentation-sdk-20261115") }, "coqualified_sdk_bridge_pin"],
    [{ sdk_verification_sha256: "0".repeat(64) }, "coqualified_sdk_receipt_pin"],
    [{ sdk_source: "b9991c9cd5f709fd0d4d0f8da58f6a063b139699" }, "followup_native_binding"],
  ] as const) {
    writeFileSync(modified, JSON.stringify({ ...pins, ...patch }), { mode: 0o600 });
    assert.throws(() => verifyInputs({ ...inputs, qualification: modified }), new RegExp(failure));
  }
});
