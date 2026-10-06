pub const AZURE_CLIENT_ID: &str = "";
pub const AZURE_RESOURCE: &str = "";
pub const AZURE_REDIRECT_URI: &str = "";
pub const FIREBASE_PROJECT_ID: &str = "";
pub const EMAIL_DOMAIN: &str = "";

pub fn azure_authorize_url() -> String {
    format!("")
}

pub fn azure_token_url() -> String {
    format!("")
}

pub fn functions_base_url() -> String {
    format!("")
}

pub fn firestore_base_url() -> String {
    format!("")
}

pub fn identity_toolkit_url(method: &str) -> String {
    format!("{method}")
}

pub fn secure_token_url() -> String {
    format!("")
}

pub(crate) fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
