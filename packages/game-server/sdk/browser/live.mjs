import { GameClient, parseSeatBootstrap } from '/browser.js';

const assert = (condition, message) => { if (!condition) throw new Error(message); };
const violations = [];
addEventListener('securitypolicyviolation', () => violations.push('CSP violation'));
document.getElementById('bootstrap').addEventListener('change', async event => {
  const output = document.getElementById('result');
  output.dataset.status = 'running';
  try {
    const file = event.target.files[0];
    assert(file && file.size <= 4096, 'Missing or oversized seat file');
    const bootstrap = parseSeatBootstrap(await file.text());
    event.target.value = ''; // Do not retain even the picker selection after import.
    const config = await (await fetch('/configuration.json', { cache: 'no-store' })).json();
    assert(new URL(bootstrap.server_origin).origin === config.upstream, 'Bootstrap origin is not the allowlisted upstream');
    const sdk = new GameClient({ baseURL: location.origin, matchId: bootstrap.match_id, capability: bootstrap.capability });
    const manifest = await sdk.manifest();
    const resume = await sdk.resume();
    for (const value of [manifest, resume]) {
      for (const key of ['match_id', 'engine_build_id', 'server_build_id', 'catalog_sha256', 'protocol_version']) assert(value[key] === bootstrap[key], 'Host identity mismatch');
    }
    assert(resume.viewer === bootstrap.viewer, 'Authenticated viewer mismatch');
    assert(resume.state.status === 'available', 'Opening observation unavailable');
    assert(resume.state.observation.view.viewer === bootstrap.viewer, 'Observation viewer mismatch');
    const page = await sdk.poll({ after: resume.next, waitMs: 0 });
    assert(page.entries.length === 0 && page.next === resume.next, 'Read-only opening boundary changed');
    const second = await sdk.resume();
    assert(JSON.stringify(second) === JSON.stringify(resume), 'Read-only resume changed committed state');
    assert(violations.length === 0, 'Production proof violated CSP');
    // Return only public identity and bounded counters. No hand, token, capability
    // or raw response is emitted to the DOM, runner, console or persistent storage.
    output.textContent = JSON.stringify({ status: 'passed', viewer: resume.viewer,
      engine_build_id: resume.engine_build_id, server_build_id: resume.server_build_id,
      catalog_sha256: resume.catalog_sha256, next: resume.next,
      observation_version: resume.state.observation.observation_version,
      decision: resume.state.observation.decision?.request ?? null,
      profile: manifest.profile, stream_entries: page.entries.length, csp_violations: violations.length }, null, 2);
    output.dataset.status = 'passed';
  } catch {
    // Never echo file data, network error bodies, browser request objects or secrets.
    output.textContent = 'Real host proof failed; inspect source-qualified server and test evidence.';
    output.dataset.status = 'failed';
  }
});
