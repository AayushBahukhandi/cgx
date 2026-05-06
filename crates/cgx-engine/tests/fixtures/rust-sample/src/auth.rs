use crate::db;

pub struct AuthService {
    pub session_timeout: u64,
}

impl AuthService {
    pub fn new() -> Self {
        Self { session_timeout: 3600 }
    }

    pub fn login(&self, email: &str, password: &str) -> bool {
        db::find_user(email).is_some()
    }

    pub fn logout(&self, session_id: &str) -> bool {
        db::delete_session(session_id)
    }
}

pub fn validate_token(token: &str) -> bool {
    token.len() == 64
}
