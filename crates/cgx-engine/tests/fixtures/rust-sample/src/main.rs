mod auth;
mod db;

fn main() {
    let service = auth::AuthService::new();
    let result = service.login("test@test.com", "password");
    println!("Login result: {}", result);
}
