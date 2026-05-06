import { AuthService, validateToken } from './auth';
import { UserService } from './user';

const auth = new AuthService();
const users = new UserService();

export async function handleLogin(email: string, password: string) {
  return await auth.login(email, password);
}

export async function handleGetUser(id: string, token: string) {
  if (!validateToken(token)) throw new Error('Invalid token');
  return await users.getUser(id);
}

export async function handleDeleteUser(id: string) {
  return await users.deleteUser(id);
}
