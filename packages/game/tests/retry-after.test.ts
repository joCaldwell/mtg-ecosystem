import assert from 'node:assert/strict';
import test from 'node:test';
import { MAX_READ_RETRY_MS, retryAfterMilliseconds } from '../web/src/session/retry.ts';

test('Retry-After accepts bounded delay-seconds and HTTP dates without treating malformed numbers as dates', () => {
  const now = Date.parse('Sun, 15 Nov 2026 02:00:00 GMT');
  assert.equal(retryAfterMilliseconds('35', now), 35_000);
  assert.equal(retryAfterMilliseconds(' 0 ', now), 0);
  assert.equal(retryAfterMilliseconds('Sun, 15 Nov 2026 02:00:35 GMT', now), 35_000);
  assert.equal(retryAfterMilliseconds('Sun, 15 Nov 2026 01:00:00 GMT', now), 0);
  assert.equal(retryAfterMilliseconds('9999999999999999999999999999999999999', now), MAX_READ_RETRY_MS);
  for (const value of [null, '', '-1', '1.5', '1e3', 'Infinity', 'invalid', '4'.repeat(129)]) {
    assert.equal(retryAfterMilliseconds(value, now), null, String(value));
  }
});
