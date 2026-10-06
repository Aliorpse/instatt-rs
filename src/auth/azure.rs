use serde::Deserialize;
use serde_json::Value;

use crate::{
    config,
    error::{Error, Result},
};

#[derive(Debug, Clone)]
pub(crate) struct AzureToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

pub fn authorize_url(upn: &str) -> String {
    format!(
        "{}?client_id={}&response_type=code&resource={}&redirect_uri={}&login_hint={}",
        config::azure_authorize_url(),
        config::encode(config::AZURE_CLIENT_ID),
        config::encode(config::AZURE_RESOURCE),
        config::encode(config::AZURE_REDIRECT_URI),
        config::encode(upn)
    )
}

pub(crate) fn parse_callback(callback_url: &str) -> Result<String> {
    let url = reqwest::Url::parse(callback_url.trim()).map_err(|error| Error::AzureAuth {
        code: "invalid_callback_url".into(),
        description: error.to_string(),
    })?;
    let mut code = None;
    let mut error = None;
    let mut description = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            "error_description" => description = Some(value.into_owned()),
            _ => {}
        }
    }
    if let Some(code) = code.filter(|value| !value.is_empty()) {
        return Ok(code);
    }
    Err(Error::AzureAuth {
        code: error.unwrap_or_else(|| "missing_code".into()),
        description: description
            .unwrap_or_else(|| "callback URL did not contain an authorization code".into()),
    })
}

pub(crate) async fn exchange_code(http: &reqwest::Client, code: &str) -> Result<AzureToken> {
    let form = [
        ("grant_type", "authorization_code"),
        ("client_id", config::AZURE_CLIENT_ID),
        ("resource", config::AZURE_RESOURCE),
        ("redirect_uri", config::AZURE_REDIRECT_URI),
        ("code", code),
    ];
    parse(
        http.post(config::azure_token_url())
            .form(&form)
            .send()
            .await?,
    )
    .await
}

pub(crate) async fn refresh(http: &reqwest::Client, refresh_token: &str) -> Result<AzureToken> {
    let form = [
        ("grant_type", "refresh_token"),
        ("client_id", config::AZURE_CLIENT_ID),
        ("resource", config::AZURE_RESOURCE),
        ("refresh_token", refresh_token),
    ];
    parse(
        http.post(config::azure_token_url())
            .form(&form)
            .send()
            .await?,
    )
    .await
}

async fn parse(response: reqwest::Response) -> Result<AzureToken> {
    let status = response.status();
    let body = response.text().await?;
    let value: Value = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(_) if !status.is_success() => {
            return Err(Error::UnexpectedResponse {
                endpoint: "azure_token",
                status: status.as_u16(),
                body,
            });
        }
        Err(error) => return Err(Error::Json(error)),
    };
    if status.is_success() {
        return Ok(AzureToken {
            access_token: value
                .get("access_token")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(Error::MissingField("access_token"))?,
            refresh_token: value
                .get("refresh_token")
                .and_then(Value::as_str)
                .map(str::to_owned),
        });
    }
    let parsed = serde_json::from_value(value).unwrap_or(AzureErrorBody {
        error: "unknown_error".into(),
        error_description: None,
    });
    Err(Error::AzureAuth {
        code: parsed.error,
        description: parsed.error_description.unwrap_or_default(),
    })
}

#[derive(Deserialize)]
struct AzureErrorBody {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}
