// One HTTP boundary, shared by the small reader client and the editor.
export class ApiError extends Error {
  status: number;
  error: string;
  constructor(status: number, error: string) { super(error); this.status = status; this.error = error; }
}

export async function request<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const res = await fetch(path, {
    method,
    credentials: "same-origin",
    headers: body !== undefined ? { "Content-Type": "application/json" } : undefined,
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
  const data = await res.json().catch(() => { throw new ApiError(res.status, "The server returned invalid JSON. Please retry."); });
  return data as T;
}

