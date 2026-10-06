pub(crate) mod azure;
pub(crate) mod firebase;

pub fn upn(username: &str) -> String {
    let username = username.trim();
    if username.contains('@') {
        username.to_owned()
    } else {
        format!("{username}@{}", crate::config::EMAIL_DOMAIN)
    }
}
