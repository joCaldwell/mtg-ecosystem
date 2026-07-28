// Every user-facing failure extends AppError and carries its HTTP status, so
// the server's catch is a single instanceof and a new error type never needs
// a new mapper line. This is a leaf module: anything (db.ts included) may
// throw a typed error without importing from deck/.
export class AppError extends Error {
  status: number;
  constructor(message: string, status = 400) {
    super(message);
    this.status = status;
  }
}

// Domain-rule violations from deck/ (bad input, missing rows → 404, …).
export class ServiceError extends AppError {}
