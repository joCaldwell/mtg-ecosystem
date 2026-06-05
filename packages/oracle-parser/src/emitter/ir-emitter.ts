// packages/oracle-parser/src/emitter/ir-emitter.ts
// AST to JSON Card IR emitter

export class IREmitter {
  constructor() {}
  
  emitSet(setMetadata: any, cards: any[]): string {
    return JSON.stringify({
      schema_version: "0.1.0",
      ...setMetadata,
      cards
    }, null, 2);
  }
}
