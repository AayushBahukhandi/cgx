import { query } from './db';

export class UserService {
  async getUser(id: string) {
    return await query('SELECT * FROM users WHERE id = ?', [id]);
  }

  async deleteUser(id: string) {
    await query('DELETE FROM users WHERE id = ?', [id]);
  }
}

export function hashPassword(password: string): string {
  return Buffer.from(password).toString('base64');
}
function _neverCalledPrivate(): void { console.log("dead"); }
export function unusedExportedHelper(x: number): number { return x * 2; }

// Duplicate of hashPassword for testing clone detection
export function encryptPassword(password: string): string {
  return Buffer.from(password).toString('base64');
}
