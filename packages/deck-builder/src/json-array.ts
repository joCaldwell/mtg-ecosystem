import { createReadStream } from "node:fs";

/** Incremental JSON array of objects. Only a single element is held in JS heap. */
export async function* readJsonArray(path: string): AsyncGenerator<unknown> {
  let state: "start" | "value" | "comma" | "end" = "start";
  let object = "", depth = 0, quoted = false, escaped = false, afterComma = false;
  for await (const chunk of createReadStream(path, { encoding: "utf8" })) {
    for (const ch of chunk) {
      if (depth) {
        object += ch;
        if (quoted) {
          if (escaped) escaped = false;
          else if (ch === "\\") escaped = true;
          else if (ch === '"') quoted = false;
        } else if (ch === '"') quoted = true;
        else if (ch === "{" || ch === "[") depth++;
        else if (ch === "}" || ch === "]") depth--;
        if (!depth) {
          yield JSON.parse(object);
          object = "";
          state = "comma";
        }
        continue;
      }
      if (/\s/.test(ch)) continue;
      if (state === "start" && ch === "[") state = "value";
      else if (state === "value" && ch === "{") { object = ch; depth = 1; afterComma = false; }
      else if ((state === "comma" || (state === "value" && !afterComma)) && ch === "]") state = "end";
      else if (state === "comma" && ch === ",") { state = "value"; afterComma = true; }
      else throw new Error("Invalid bulk JSON: expected an array of card objects");
    }
  }
  if (state !== "end" || depth) throw new Error("Incomplete bulk JSON array");
}
