/*
The MIT License (MIT)
Copyright (c) 2024 - present, Bereket Engida

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the “Software”), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
*/
// Adapted from Better Auth's MIT-licensed v1.7.13 packages/cimd/src/node.ts:
// https://github.com/better-auth/better-auth/blob/v1.7.13/packages/cimd/src/node.ts
// Its verbatim first DNS answer can pin unreachable IPv6 on IPv4-only hosts.
// Prefer IPv4 while preserving its documented security contract: inspect ALL
// answers, resolve ONCE, pin ONE checked address, preserve Host/SNI and TLS
// verification, never follow redirects. Never replace this with ordinary fetch.
import { isPublicRoutableHost } from "@better-auth/core/utils/host";
import { lookup } from "node:dns/promises";
import { request } from "node:https";
import { isIP } from "node:net";
import { Readable } from "node:stream";

type Resolver = (hostname: string) => Promise<import("node:dns").LookupAddress[]>;
function abortable<T>(work: Promise<T>, signal: AbortSignal): Promise<T> {
  signal.throwIfAborted();
  return new Promise((resolve,reject)=>{
    const abort=()=>reject(signal.reason);
    signal.addEventListener("abort",abort,{once:true});
    work.then(resolve,reject).finally(()=>signal.removeEventListener("abort",abort));
  });
}
/** A bounded public-document cache and fetch pool, per service instance. */
export function createMetadataTransport(resolveHost: Resolver = hostname=>lookup(hostname,{all:true,order:"ipv4first"})) {
  const cache=new Map<string,{expires:number;status:number;headers:[string,string][];body:Uint8Array}>();
  let active=0;
  return async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const web=new Request(input,init),url=new URL(web.url);
    if(url.protocol!=="https:" || url.username || url.password)throw new TypeError("CIMD requires a credential-free HTTPS URL");
    if(!["GET","HEAD"].includes(web.method))throw new TypeError("CIMD only supports GET and HEAD");
    const signal=AbortSignal.any([web.signal,AbortSignal.timeout(5000)]);signal.throwIfAborted();
    const key=web.method+":"+url.href,cached=cache.get(key);
    if(cached && cached.expires>Date.now())return new Response(cached.body.slice(),{status:cached.status,headers:cached.headers});
    if(active>=4)throw new Error("CIMD fetch capacity reached; retry shortly");
    active++;
    try {
      const hostname=url.hostname.replace(/^\[|\]$/g, "");
      // getaddrinfo cannot be cancelled. Stop waiting at the caller deadline;
      // late answers are discarded and can never start a socket connection.
      const addresses=await abortable(resolveHost(hostname),signal);
      if(!addresses.length || addresses.some(a=>!isPublicRoutableHost(a.address)))throw new TypeError("CIMD requires public-routable DNS answers only");
      signal.throwIfAborted();const pinned=addresses[0];
      const response=await new Promise<Response>((resolve,reject)=>{
        const req=request(url,{
          agent:false,method:web.method,headers:{...Object.fromEntries(web.headers),host:url.host},
          servername:isIP(hostname)?undefined:hostname,signal,
          lookup:(_hostname,opts,callback)=>{if(opts.all)callback(null,[pinned]);else callback(null,pinned.address,pinned.family);},
        },res=>{
          const headers=new Headers();
          for(const [k,v]of Object.entries(res.headers))if(v!==undefined){if(Array.isArray(v))for(const item of v)headers.append(k,item);else headers.set(k,v);}
          const body=web.method==="HEAD" || [204,205,304].includes(res.statusCode??500)?null:Readable.toWeb(res) as ReadableStream<Uint8Array>;
          resolve(new Response(body,{status:res.statusCode??500,statusText:res.statusMessage,headers}));
        });req.once("error",reject);req.end();
      });
      const reader=response.body?.getReader(),chunks:Uint8Array[]=[];let size=0;
      try {if(reader)for(;;){const next=await abortable(reader.read(),signal);if(next.done)break;size+=next.value.length;if(size>65_536)throw new Error("CIMD document exceeds 64 KiB");chunks.push(next.value);}}
      catch(error){await reader?.cancel().catch(()=>{});throw error;}
      const body=new Uint8Array(size);let offset=0;for(const chunk of chunks){body.set(chunk,offset);offset+=chunk.length;}
      const headers=[...response.headers.entries()];
      if(response.status===200 && !web.headers.has("authorization") && !web.headers.has("cookie") && !/no-store/i.test(response.headers.get("cache-control")??"")){
        if(cache.size>=128)cache.delete(cache.keys().next().value!);
        cache.set(key,{expires:Date.now()+30_000,status:response.status,headers,body});
      }
      return new Response(response.body===null?null:body,{status:response.status,headers});
    } finally {active--;}
  };
}
export const fetchClientMetadata = createMetadataTransport();
