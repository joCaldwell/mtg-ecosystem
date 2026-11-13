/** Local authorization lifetime, not a wire identifier or credential. Create a
 * fresh binding object after every authorization, including reimport of a seat. */
export interface AuthorizationLifetime { readonly identity: object }
export interface Generation {
  readonly authorization: AuthorizationLifetime;
  readonly epoch: symbol;
}

/** Abort is an effect; this fence independently rejects late async completions. */
export class GenerationFence {
  #current: Generation | null = null;

  start(authorization: AuthorizationLifetime): Generation {
    if (this.#current?.authorization === authorization) return this.#current;
    this.#current = Object.freeze({ authorization, epoch: Symbol() });
    return this.#current;
  }

  isCurrent(generation: Generation): boolean { return this.#current === generation; }

  /** A late cleanup from a previous owner cannot stop its replacement. */
  stop(generation: Generation): void {
    if (this.#current === generation) this.#current = null;
  }
}
