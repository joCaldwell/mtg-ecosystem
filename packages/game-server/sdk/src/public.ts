import type { ClientConfig } from '../generated/client_config.js';
import type { PresentationDescriptor } from '../generated/presentation_descriptor.js';
import type { PresentationArtifact } from '../generated/presentation.js';
import { client_configValidator, presentation_descriptorValidator, presentationValidator } from '../generated/validators.js';
import { PRESENTATION_SCHEMA_SHA256, PRESENTATION_PRODUCER_SHA256 } from '../generated/presentation-identity.ts';
import { ROUTES } from '../generated/routes.ts';
import { ProtocolError } from './validation.ts';
import { strictJSON } from './strict-json.ts';
import { requestJSON } from './transport.ts';
export type { ClientConfig, PresentationDescriptor, PresentationArtifact };
export function parseClientConfig(text:string):ClientConfig {
  if(new TextEncoder().encode(text).length>4096) throw new ProtocolError('Client config exceeds limit');
  const value=strictJSON(text);
  if(!client_configValidator(value)) throw new ProtocolError('Invalid client config');
  let url:URL;
  try {url=new URL(value.server_origin);} catch {throw new ProtocolError('Invalid client config origin');}
  if(url.protocol!=='http:' || !['127.0.0.1','[::1]'].includes(url.hostname) || `http://${url.hostname}:${url.port||'80'}`!==value.server_origin) throw new ProtocolError('Invalid client config origin');
  return value;
}
export async function readClientConfig(baseURL:string, options:{signal?:AbortSignal;fetch?:typeof fetch}={}):Promise<ClientConfig> {
  let url:URL;
  try {url=new URL(baseURL);} catch {throw new ProtocolError('Invalid client origin');}
  if(url.protocol!=='http:' || !['127.0.0.1','[::1]'].includes(url.hostname) || url.username || url.password || url.pathname!=='/' || url.search || url.hash) throw new ProtocolError('Invalid client origin');
  const {bytes}=await requestJSON((options.fetch??globalThis.fetch).bind(globalThis),`${url.origin}${ROUTES.get_client_config.path}`,{method:'GET',headers:{Accept:'application/json'}},4096,options.signal);
  return parseClientConfig(new TextDecoder().decode(bytes));
}
export function presentationDescriptor(value:unknown):PresentationDescriptor {
  if(!presentation_descriptorValidator(value) || value.path!==`/presentation/${value.presentation_sha256}.json`
    || value.presentation_schema_sha256!==PRESENTATION_SCHEMA_SHA256 || value.producer_sha256!==PRESENTATION_PRODUCER_SHA256
    || [value.catalog_sha256,value.presentation_sha256].some(v=>v.length!==64 || /[^0-9a-f]/.test(v))) throw new ProtocolError('Invalid presentation descriptor');
  return value;
}
export async function presentationArtifact(value:unknown,bytes:Uint8Array,descriptor:PresentationDescriptor):Promise<PresentationArtifact> {
  const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new Uint8Array(bytes)))).map(v=>v.toString(16).padStart(2,'0')).join('');
  if(hash!==descriptor.presentation_sha256 || !presentationValidator(value) || value.catalog_sha256!==descriptor.catalog_sha256) throw new ProtocolError('Invalid presentation artifact');
  const seen=new Set<string>();
  for(const card of value.cards) {
    if(seen.has(card.oracle_id)) throw new ProtocolError('Invalid presentation identities'); seen.add(card.oracle_id);
    for(const [i,face] of card.faces.entries()) {
      if(face.face_index!==i) throw new ProtocolError('Invalid presentation faces');
      if(face.image_url!==null) {
        let url:URL; try {url=new URL(face.image_url);} catch {throw new ProtocolError('Invalid presentation image');}
        if(url.protocol!=='https:' || url.username || url.password) throw new ProtocolError('Invalid presentation image');
      }
    }
  }
  return value;
}
