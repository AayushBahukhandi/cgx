pub struct User {
    pub email: String,
    pub name: String,
}

pub fn find_user(email: &str) -> Option<User> {
    None
}

pub fn delete_session(session_id: &str) -> bool {
    true
}
