import { hashPassword } from './user';
import { query } from './db';

export class AuthService {
  async login(email: string, password: string): Promise<boolean> {
    const hashed = hashPassword(password);
    const result = await query('SELECT * FROM users WHERE email = ?', [email]);
    if (result.length === 0) {
      return false;
    }
    return true;
  }

  async logout(sessionId: string): Promise<void> {
    await query('DELETE FROM sessions WHERE id = ?', [sessionId]);
  }
}

export function validateToken(token: string): boolean {
  return token.length === 64;
}
// TODO: fix this later
