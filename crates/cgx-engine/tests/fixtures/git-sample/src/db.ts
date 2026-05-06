export function query(sql: string) { return []; }
export function connect() { return true; }
export function transaction(fn: Function) { return fn(); }
// alice update 1
// alice update 2
