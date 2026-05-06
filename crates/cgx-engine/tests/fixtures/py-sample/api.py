from auth import AuthService, hash_password
from models import User, Session

auth = AuthService()

def login_endpoint(email: str, password: str):
    return auth.login(email, password)

def get_user_endpoint(user_id: str):
    return User.find_by_email(user_id)

def create_session(user_id: str):
    return Session(user_id)
