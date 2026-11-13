import { ProtocolError } from './validation.ts';
import { strictJSON } from './strict-json.ts';
export class HttpError extends Error {
  readonly status: number;
  constructor(status: number) { super(`Game server returned HTTP ${status}`); this.name = 'HttpError'; this.status = status; }
}
export class TransportError extends Error {
  constructor() { super('Game server request failed; outcome may be unknown'); this.name = 'TransportError'; }
}
export function aborted(): Error { const error = new Error('Request aborted; outcome may be unknown'); error.name = 'AbortError'; return error; }
/** Shared bounded raw response boundary. No retry or session policy. */
export async function requestJSON(fetcher: typeof fetch, url: string, init: RequestInit, maximumBytes: number, signal?: AbortSignal): Promise<{value:unknown;bytes:Uint8Array}> {
  if (signal?.aborted) throw aborted();
  let response: Response;
  try { response = await fetcher(url, { ...init, ...(signal === undefined ? {} : {signal}), redirect:'error', credentials:'omit', cache:'no-store', referrerPolicy:'no-referrer' }); }
  catch { if (signal?.aborted) throw aborted(); throw new TransportError(); }
  const reject = async (error: Error): Promise<never> => { await response.body?.cancel().catch(()=>{}); throw error; };
  if (response.redirected) return reject(new ProtocolError('Unexpected redirected response'));
  if (!response.ok) return reject(new HttpError(response.status));
  if (!/^application\/json(?:\s*;|$)/i.test(response.headers.get('content-type') ?? '')) return reject(new ProtocolError('Expected JSON response'));
  const declared = Number(response.headers.get('content-length'));
  if (Number.isFinite(declared) && declared > maximumBytes) return reject(new ProtocolError('Response body exceeds limit'));
  const reader=response.body?.getReader();
  if (!reader) throw new ProtocolError('Missing JSON response body');
  let size=0; const chunks:Uint8Array[]=[];
  try {
    for (;;) {
      if (signal?.aborted) throw aborted();
      const result=await reader.read(); if (result.done) break;
      size+=result.value.byteLength; if (size>maximumBytes) throw new ProtocolError('Response body exceeds limit'); chunks.push(result.value);
    }
    const bytes=new Uint8Array(size); let offset=0;
    for (const chunk of chunks) { bytes.set(chunk,offset); offset+=chunk.length; }
    const text=new TextDecoder('utf-8',{fatal:true}).decode(bytes);
    return {value:strictJSON(text),bytes};
  } catch(error) {
    await reader.cancel().catch(()=>{});
    if(signal?.aborted) throw aborted();
    if(error instanceof ProtocolError) throw error;
    throw new ProtocolError('Invalid JSON response');
  } finally {reader.releaseLock();}
}
