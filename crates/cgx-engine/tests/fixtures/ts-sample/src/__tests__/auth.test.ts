import { AuthService } from '../auth';
const service = new AuthService();
test('login returns boolean', () => {
  expect(typeof service.login('a', 'b')).toBe('boolean');
});
