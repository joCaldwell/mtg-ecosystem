import type { IncomingMessage } from "node:http";
import { AppError } from "./errors.ts";
const bodies = new WeakMap<IncomingMessage, Buffer>();
/** Buffer before acquiring the shared DB reservation: a slow upload must not
 * stop other people's reads. Both front doors and the pinned Node auth bridge
 * reuse the exact bytes (including repeated OAuth form parameters). */
export function readRequestBody(req: IncomingMessage): Promise<Buffer> {
  const cached=bodies.get(req);if(cached)return Promise.resolve(cached);
  if(["GET","HEAD"].includes(req.method ?? "GET"))return Promise.resolve(Buffer.alloc(0));
  return new Promise((resolve,reject)=>{
    const chunks:Buffer[]=[];let size=0,failed=false;
    const timer=setTimeout(()=>fail(new AppError("Request body timed out",408)),10_000);timer.unref();
    const fail=(e:Error)=>{if(failed)return;failed=true;clearTimeout(timer);chunks.length=0;reject(e);};
    req.on("data",(chunk:Buffer)=>{
      if(failed)return;size+=chunk.length;
      if(size>1024*1024)return fail(new AppError("Request body too large",413));
      chunks.push(chunk);
    });
    req.on("error",fail);req.on("aborted",()=>fail(new AppError("Request aborted",400)));
    req.on("end",()=>{clearTimeout(timer);if(!failed){const body=Buffer.concat(chunks);bodies.set(req,body);resolve(body);}});
  });
}
