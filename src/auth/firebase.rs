use base64::Engine;
use serde_json::{Value, json};

use crate::{
    config,
    error::{Error, Result},
};

#[derive(Debug, Clone)]
pub(crate) struct FirebaseSession {
    pub id_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct FirebaseIdentity {
    pub student_id: String,
    pub privilege_type: i64,
}

pub(crate) async fn exchange_azure_token(
    http: &reqwest::Client,
    access_token: &str,
) -> Result<String> {
    const FUNCTION: &str = "userLogin";
    let response = http
        .post(format!("{}/{FUNCTION}", config::functions_base_url()))
        .json(&json!({"data": {"token": access_token}}))
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        return Err(Error::function_response(FUNCTION, status, &body));
    }

    let body: Value = serde_json::from_str(&body)?;
    if let Some(error) = body.get("error") {
        return Err(Error::FunctionFailed {
            function: FUNCTION,
            message: error_message(error).unwrap_or_else(|| "unknown error".to_owned()),
        });
    }

    let result = body.get("result").ok_or(Error::MissingField("result"))?;
    let status_code = result
        .get("statusCode")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if status_code != 200 {
        return Err(Error::AzureTokenRejected(status_code));
    }

    result
        .get("token")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(Error::MissingField("token"))
}

pub(crate) async fn sign_in_with_custom_token(
    http: &reqwest::Client,
    custom_token: &str,
) -> Result<FirebaseSession> {
    let response = http
        .post(config::identity_toolkit_url("signInWithCustomToken"))
        .json(&json!({"token": custom_token, "returnSecureToken": true}))
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        return Err(Error::Firebase(describe_auth_error(&body, status)));
    }

    let body: Value = serde_json::from_str(&body)?;
    Ok(FirebaseSession {
        id_token: string_field(&body, "idToken")?,
        refresh_token: string_field(&body, "refreshToken")?,
        expires_in: number(&body, "expiresIn").unwrap_or(3600),
    })
}

pub(crate) async fn refresh_id_token(
    http: &reqwest::Client,
    refresh_token: &str,
) -> Result<FirebaseSession> {
    let response = http
        .post(config::secure_token_url())
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        return Err(refresh_error(&body, status));
    }

    let body: Value = serde_json::from_str(&body)?;
    Ok(FirebaseSession {
        id_token: body
            .get("id_token")
            .and_then(Value::as_str)
            .or_else(|| body.get("access_token").and_then(Value::as_str))
            .ok_or(Error::MissingField("id_token"))?
            .to_owned(),
        refresh_token: body
            .get("refresh_token")
            .and_then(Value::as_str)
            .unwrap_or(refresh_token)
            .to_owned(),
        expires_in: number(&body, "expires_in").unwrap_or(3600),
    })
}

pub(crate) fn decode_identity(id_token: &str) -> Result<FirebaseIdentity> {
    let payload = id_token
        .split('.')
        .nth(1)
        .ok_or(Error::MissingField("id_token"))?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|error| Error::Firebase(format!("invalid JWT: {error}")))?;
    let value: Value = serde_json::from_slice(&bytes)?;

    Ok(FirebaseIdentity {
        student_id: value
            .get("ID")
            .and_then(Value::as_str)
            .ok_or(Error::MissingField("ID"))?
            .to_owned(),
        privilege_type: value
            .get("privilegeType")
            .and_then(Value::as_i64)
            .ok_or(Error::MissingField("privilegeType"))?,
    })
}

fn string_field(value: &Value, field: &'static str) -> Result<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(Error::MissingField(field))
}

fn number(value: &Value, field: &str) -> Option<u64> {
    value.get(field).and_then(Value::as_u64).or_else(|| {
        value
            .get(field)
            .and_then(Value::as_str)
            .and_then(|value| value.parse().ok())
    })
}

fn error_message(error: &Value) -> Option<String> {
    error
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| error.as_str().map(str::to_owned))
}

fn refresh_error(body: &str, status: reqwest::StatusCode) -> Error {
    let value = serde_json::from_str::<Value>(body).ok();
    let error = value.as_ref().and_then(|value| value.get("error"));
    let code = error
        .and_then(|error| error.get("errors"))
        .and_then(Value::as_array)
        .and_then(|errors| errors.first())
        .and_then(|error| error.get("reason"))
        .and_then(Value::as_str)
        .or_else(|| {
            error
                .and_then(|error| error.get("status"))
                .and_then(Value::as_str)
        });
    let message = error
        .and_then(error_message)
        .unwrap_or_else(|| format!("HTTP {status}"));

    match code {
        Some(
            "TOKEN_EXPIRED"
            | "INVALID_REFRESH_TOKEN"
            | "USER_DISABLED"
            | "USER_NOT_FOUND"
            | "INVALID_GRANT",
        ) => Error::FirebaseRefreshRejected(message),
        _ => Error::Firebase(message),
    }
}

fn describe_auth_error(body: &str, status: reqwest::StatusCode) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| value.get("error").and_then(error_message))
        .unwrap_or_else(|| format!("HTTP {status}"))
}
