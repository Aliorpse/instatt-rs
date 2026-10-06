use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("failed to build the HTTP client: {0}")]
    HttpClientBuild(reqwest::Error),

    #[error("could not decode JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Azure authentication failed ({code}): {description}")]
    AzureAuth { code: String, description: String },

    #[error("the backend rejected the Azure access token (status code {0})")]
    AzureTokenRejected(i64),

    #[error("Firebase authentication failed: {0}")]
    Firebase(String),

    #[error("Firebase rejected the refresh token: {0}")]
    FirebaseRefreshRejected(String),

    #[error("this library only supports student accounts (privilegeType={privilege_type})")]
    NotStudentAccount { privilege_type: i64 },

    #[error("cloud function `{function}` failed: {message}")]
    FunctionFailed {
        function: &'static str,
        message: String,
    },

    #[error("cloud function `{function}` returned status code {status}")]
    FunctionStatus { function: &'static str, status: i64 },

    #[error("{endpoint} returned HTTP {status}: {body}")]
    UnexpectedResponse {
        endpoint: &'static str,
        status: u16,
        body: String,
    },

    #[error("Firestore request failed: {0}")]
    Firestore(String),

    #[error("field `{0}` is missing or has the wrong type")]
    MissingField(&'static str),

    #[error("field `{field}` is invalid: {message}")]
    InvalidField {
        field: &'static str,
        message: &'static str,
    },

    #[error("the session expired and no refresh token is available; sign in again")]
    NotAuthenticated,

    #[error("no device UID is available")]
    NoDeviceUid,

    #[error("a device was registered less than 24 hours ago")]
    DeviceRegistrationCooldown,

    #[error("device registration outcome is unknown (result key {result_key}): {source}")]
    DeviceRegistrationUnknown {
        result_key: String,
        #[source]
        source: Box<Error>,
    },
}

impl Error {
    pub(crate) fn function_response(
        function: &'static str,
        status: reqwest::StatusCode,
        body: &str,
    ) -> Self {
        let message = serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|value| value.get("error").and_then(error_message));

        message
            .map(|message| Self::FunctionFailed { function, message })
            .unwrap_or_else(|| Self::UnexpectedResponse {
                endpoint: function,
                status: status.as_u16(),
                body: body.to_owned(),
            })
    }
}

fn error_message(error: &Value) -> Option<String> {
    error
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| error.as_str().map(str::to_owned))
}

pub type Result<T> = std::result::Result<T, Error>;
