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

export async function fetchClientMetadata(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  const web = new Request(input,init), url = new URL(web.url);
  if (url.protocol !== "https:" || url.username || url.password) throw new TypeError("CIMD requires a credential-free HTTPS URL");
  if (!["GET","HEAD"].includes(web.method)) throw new TypeError("CIMD only supports GET and HEAD");
  const hostname = url.hostname.replace(/^\[|\]$/g, "");
  const addresses = await lookup(hostname,{all:true,order:"ipv4first"});
  if (!addresses.length || addresses.some(a=>!isPublicRoutableHost(a.address))) throw new TypeError("CIMD requires public-routable DNS answers only");
  const pinned=addresses[0];
  return new Promise((resolve,reject)=>{
    const req=request(url,{
      agent:false, method:web.method, headers:{...Object.fromEntries(web.headers),host:url.host},
      servername:isIP(hostname)?undefined:hostname, signal:web.signal,
      lookup: (_hostname,opts,callback) => {
        if(opts.all) callback(null,[pinned]);else callback(null,pinned.address,pinned.family);
      },
    },res=>{
      const headers=new Headers();
      for(const [key,value] of Object.entries(res.headers)) if(value!==undefined) {
        if(Array.isArray(value))for(const v of value)headers.append(key,v);else headers.set(key,value);
      }
      const body = web.method === "HEAD" || [204,205,304].includes(res.statusCode ?? 500) ? null : Readable.toWeb(res) as ReadableStream<Uint8Array>;
      resolve(new Response(body,{status:res.statusCode ?? 500,statusText:res.statusMessage,headers}));
    });
    req.once("error",reject);req.end();
  });
}
