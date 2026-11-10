// Reserve route names before public /u/:handle URLs ship. The tombstone name
// cannot be registered or selected by a real account.
export const RESERVED_HANDLES = new Set(["admin","api","settings","signin","privacy","terms","d","u","mcp","deleted_account"]);
