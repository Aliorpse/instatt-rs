use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::{
    config,
    error::{Error, Result},
};

#[derive(Debug, Deserialize)]
pub(crate) struct Document {
    name: String,
    #[serde(default)]
    fields: HashMap<String, FirestoreValue>,
}

impl Document {
    pub(crate) fn id(&self) -> &str {
        self.name.rsplit('/').next().unwrap_or_default()
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn fields(&self) -> &HashMap<String, FirestoreValue> {
        &self.fields
    }

    pub(crate) fn get_str(&self, field: &str) -> Option<&str> {
        self.fields.get(field).and_then(FirestoreValue::as_str)
    }

    pub(crate) fn get_i64(&self, field: &str) -> Option<i64> {
        self.fields.get(field).and_then(FirestoreValue::as_i64)
    }
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub(crate) struct FirestoreValue(Value);

impl FirestoreValue {
    pub(crate) fn as_str(&self) -> Option<&str> {
        self.0
            .get("stringValue")
            .or_else(|| self.0.get("referenceValue"))
            .and_then(Value::as_str)
    }

    fn as_i64(&self) -> Option<i64> {
        self.0
            .get("integerValue")
            .and_then(Value::as_str)
            .and_then(|value| value.parse().ok())
            .or_else(|| {
                self.0
                    .get("doubleValue")
                    .and_then(Value::as_f64)
                    .filter(|value| value.is_finite() && value.fract() == 0.0)
                    .map(|value| value as i64)
            })
    }
}

pub(crate) async fn get_document(
    http: &reqwest::Client,
    bearer: &str,
    path: &str,
) -> Result<Option<Document>> {
    let response = http
        .get(format!("{}/{path}", config::firestore_base_url()))
        .bearer_auth(bearer)
        .send()
        .await?;
    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let body = response.text().await?;
    if !status.is_success() {
        return Err(Error::Firestore(describe(&body, status)));
    }

    Ok(Some(serde_json::from_str(&body)?))
}

pub(crate) async fn run_query(
    http: &reqwest::Client,
    bearer: Option<&str>,
    parent: &str,
    query: Value,
) -> Result<Vec<Document>> {
    let base = config::firestore_base_url();
    let url = if parent.is_empty() {
        format!("{base}:runQuery")
    } else {
        format!("{base}/{parent}:runQuery")
    };
    let mut request = http
        .post(url)
        .json(&serde_json::json!({"structuredQuery": query}));
    if let Some(token) = bearer {
        request = request.bearer_auth(token);
    }

    let response = request.send().await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(Error::Firestore(describe(&body, status)));
    }

    serde_json::from_str::<Value>(&body)?
        .as_array()
        .ok_or_else(|| Error::Firestore("runQuery did not return an array".into()))?
        .iter()
        .filter_map(|row| row.get("document").cloned())
        .map(|row| serde_json::from_value(row).map_err(Error::from))
        .collect()
}

pub(crate) async fn batch_get(
    http: &reqwest::Client,
    bearer: &str,
    paths: &[String],
) -> Result<Vec<Option<Document>>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }

    let documents: Vec<_> = paths.iter().map(|path| full_name(path)).collect();
    let response = http
        .post(format!("{}:batchGet", config::firestore_base_url()))
        .bearer_auth(bearer)
        .json(&serde_json::json!({"documents": documents}))
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(Error::Firestore(describe(&body, status)));
    }

    let rows: Vec<Value> = serde_json::from_str(&body)?;
    let mut found = HashMap::new();
    for row in rows {
        if let Some(document) = row.get("found") {
            let document: Document = serde_json::from_value(document.clone())?;
            found.insert(document.name().to_owned(), Some(document));
        } else if let Some(path) = row.get("missing").and_then(Value::as_str) {
            found.insert(path.to_owned(), None);
        }
    }

    paths
        .iter()
        .map(|path| {
            found
                .remove(&full_name(path))
                .ok_or_else(|| Error::Firestore(format!("batchGet omitted `{path}`")))
        })
        .collect()
}

fn full_name(path: &str) -> String {
    format!(
        "projects/{}/databases/(default)/documents/{path}",
        config::FIREBASE_PROJECT_ID
    )
}

fn describe(body: &str, status: reqwest::StatusCode) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(|message| format!("{status}: {message}"))
        })
        .unwrap_or_else(|| format!("{status}: {body}"))
}
