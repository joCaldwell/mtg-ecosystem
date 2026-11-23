export declare class HttpError extends Error {
    readonly status: number;
    constructor(status: number);
}
export declare class TransportError extends Error {
    constructor();
}
export declare function aborted(): Error;
/** Shared bounded raw response boundary. No retry or session policy. */
export declare function requestJSON(fetcher: typeof fetch, url: string, init: RequestInit, maximumBytes: number, signal?: AbortSignal): Promise<{
    value: unknown;
    bytes: Uint8Array;
}>;
