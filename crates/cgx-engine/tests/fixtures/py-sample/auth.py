from models import User

class AuthService:
    def login(self, email: str, password: str) -> bool:
        user = User.find_by_email(email)
        return user is not None

    def logout(self, session_id: str) -> None:
        pass

def hash_password(password: str) -> str:
    return password[::-1]
