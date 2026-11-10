// One HTTP boundary, shared by the small reader client and the editor.
export class ApiError extends Error {
  status: number;
  error: string;
  constructor(status: number, error: string) { super(error); this.status = status; this.error = error; }
}

export async function request<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  // Only these synchronous commands persist receipts. Never retry credential
  // creation or authentication, whose once-shown secrets must not be stored.
  const command = !["GET", "HEAD"].includes(method) && /^\/api\/(?:decks(?:\/|$)|memories(?:\/|$)|profile(?:\/|$)|document-edits(?:\/|$)|demo$|public\/decks\/.+\/fork$)/.test(path);
  const key = command ? crypto.randomUUID() : undefined;
  const headers: Record<string, string> = {};
  if (body !== undefined) headers["Content-Type"] = "application/json";
  if (key) headers["Idempotency-Key"] = key;
  const send = async (): Promise<T> => {
    const res = await fetch(path, {
      method,
      credentials: "same-origin",
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
      signal,
    });
    const json = res.headers.get("content-type")?.split(";")[0].trim() === "application/json";
    if (!res.ok) {
      const data = json ? await res.json().catch(() => null) : null;
      if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent("api-error", { detail: { status: res.status, retryAfter: Number(res.headers.get("Retry-After") ?? 60), message: data?.error } }));
      throw new ApiError(res.status, typeof data?.error === "string" ? data.error : `HTTP ${res.status}`);
    }
    if (!json) throw new ApiError(res.status, "The server returned an unexpected response. Please retry.");
    const data = await res.json();
    // The receipt is the original outcome. Refresh a deck composite before
    // displaying it: another actor may have edited it since that outcome.
    if (data?.command?.replayed && data.state?.deck?.id)
      data.state = (await request<{ state: unknown }>("GET", `/api/decks/${data.state.deck.id}`, undefined, signal)).state;
    return data as T;
  };
  try { return await send(); }
  catch (error) {
    // A dropped response can follow a committed write. Retry the same command
    // once with its original identity; HTTP errors already have a clear result.
    if (!key || signal?.aborted || error instanceof ApiError) throw error;
    return send();
  }
}

