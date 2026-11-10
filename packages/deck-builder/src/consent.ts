// MCP 2026-07-28 requires a visible redirect hostname and localhost warnings:
// https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization/security-considerations
import type { ConsentIdentity } from "./oauth-policy.ts";
export function consentHtml(client: ConsentIdentity, stylesheets: string[] = []): string {
  const escape = (s: string) => s.replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));
  const name=client.name;
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Connect ${escape(name)} · Deck builder</title>${stylesheets.map(s=>`<link rel="stylesheet" href="${escape(s)}">`).join("")}<style>main{display:grid;gap:var(--s-4);border:1px solid var(--line);border-radius:var(--r-md)}h1{font-size:var(--text-xl)}p{margin:0}button{padding:var(--s-3) var(--s-5);border:1px solid var(--line-strong);border-radius:var(--r-sm);background:var(--surface);color:var(--text);font:inherit}button:disabled{opacity:.5}button:focus-visible{outline:2px solid var(--sel)}.actions{display:flex;gap:var(--s-3);margin-top:var(--s-3)}</style></head><body><main style="max-width:var(--page-md,42rem);margin:var(--s-8,2rem) auto;padding:var(--s-5,1.5rem)"><h1>Connect ${escape(name)}</h1><p><strong>${client.identityHost ? `Client identity: ${escape(client.identityHost)}` : "Unverified app — its name was chosen by the app itself"}</strong></p><h2>Redirect destination: ${escape(client.redirectHost)}</h2><p>${escape(client.redirectUri)}</p>${client.loopback ? "<p><strong>Localhost redirect: another app on your computer could impersonate this client. Continue only if you started this connection in a trusted local app.</strong></p>" : ""}<p>Give this client access to your deck-builder account?</p><p><strong>deck-builder</strong>: read your decks and knowledge, and make attributed edits and proposals under the same permissions as your other agents. Paper changes still require your review.</p><p>Offline access lets the client renew this permission until you disconnect it.</p><p>Only approve clients you trust.</p><div class="actions"><button id="approve">Approve access</button><button id="deny">Deny</button></div><p id="error" role="alert"></p><p><a href="/privacy">Privacy</a> · <a href="/terms">Terms</a></p></main><script src="/consent.js" defer></script></body></html>`;
}
export const consentScript = `
for (const [id,accept] of [['approve',true],['deny',false]]) document.getElementById(id).addEventListener('click',async()=>{
  const buttons=[...document.querySelectorAll('button')];buttons.forEach(b=>b.disabled=true);
  try {
    const response=await fetch('/api/auth/oauth2/consent',{method:'POST',credentials:'same-origin',headers:{'Content-Type':'application/json','Accept':'application/json'},body:JSON.stringify({accept,oauth_query:location.search.slice(1)})});
    const result=await response.json();if(!response.ok || !result.url)throw Error(result.message || result.error || 'Consent failed. Start the connection again.');
    location.assign(result.url);
  } catch(e){document.getElementById('error').textContent=e.message;buttons.forEach(b=>b.disabled=false);}
});`;
