class User:
    def __init__(self, email: str, name: str):
        self.email = email
        self.name = name

    @classmethod
    def find_by_email(cls, email: str):
        return None

class Session:
    def __init__(self, user_id: str):
        self.user_id = user_id

    def is_valid(self) -> bool:
        return True
