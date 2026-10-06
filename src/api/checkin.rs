use std::time::Duration;

use chrono::Timelike;
use serde_json::{Value, json};

use super::firestore;
use crate::{
    client::InstattClient,
    error::{Error, Result},
    model::{Class, date_to_wire},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckinOutcome {
    Accepted,
    AcceptedPending,
    Unauthorized,
    DeviceNotInVenue,
    DeviceNotRegistered,
    Unknown { status_code: i64 },
}

impl CheckinOutcome {
    pub fn status_code(self) -> i64 {
        match self {
            Self::Accepted => 200,
            Self::AcceptedPending => 202,
            Self::Unauthorized => 401,
            Self::DeviceNotInVenue => 403,
            Self::DeviceNotRegistered => 409,
            Self::Unknown { status_code } => status_code,
        }
    }

    pub fn is_success(self) -> bool {
        matches!(self, Self::Accepted | Self::AcceptedPending)
    }
}

impl InstattClient {
    /// Load current devicd UID from Firebase.
    pub async fn load_device_uid(&self) -> Result<Option<String>> {
        let _transition = self.device_transition().await;
        let bearer = self.bearer().await?;
        let document = firestore::get_document(
            &self.http,
            &bearer,
            &format!("students/{}", self.student_id()),
        )
        .await?;
        let value = document.and_then(|document| document.get_str("deviceUID").map(str::to_owned));

        *self.device_uid.write().await = value.clone();
        Ok(value)
    }

    /// Set custom device uid, which won't sync to Firebase.
    pub async fn set_device_uid(&self, new_device_uid: &str) {
        *self.device_uid.write().await = Some(new_device_uid.to_owned());
    }

    /// Publishes a device UID only after confirmation. Unknown outcomes clear the cache.
    pub async fn register_device(&self) -> Result<String> {
        let _transition = self.device_transition().await;
        *self.device_uid.write().await = None;
        let response = self
            .call_function(
                "registerDevice",
                json!({
                    "studentID": self.student_id(),
                }),
            )
            .await?;

        match status(&response) {
            200 => {}
            304 => return Err(Error::DeviceRegistrationCooldown),
            code => {
                return Err(Error::FunctionStatus {
                    function: "registerDevice",
                    status: code,
                });
            }
        }

        let device_uid = response
            .get("tempDeviceUID")
            .and_then(Value::as_str)
            .ok_or(Error::MissingField("tempDeviceUID"))?
            .to_owned();

        let result_key = format!("{}{}", self.student_id(), Self::unix_seconds());
        let payload = json!({
            "studentID": self.student_id(),
            "deviceUID": device_uid,
            "resultKey": result_key,
        });

        let confirmation = self.call_function("registerDeviceSuccess", payload).await;
        if !matches!(&confirmation, Ok(response) if status(response) == 200) {
            let original = match confirmation {
                Ok(response) => Error::FunctionStatus {
                    function: "registerDeviceSuccess",
                    status: status(&response),
                },
                Err(error) => error,
            };
            registration_result(
                result_key.clone(),
                original,
                self.confirm_registration(&result_key).await,
            )?;
        }

        *self.device_uid.write().await = Some(device_uid.clone());
        Ok(device_uid)
    }

    pub async fn sign_attendance(&self, class: &Class, bssid: &str) -> Result<CheckinOutcome> {
        let cached_uid = self.device_uid.read().await.clone();
        let device_uid = match cached_uid {
            Some(value) => value,
            None => self.load_device_uid().await?.ok_or(Error::NoDeviceUid)?,
        };
        let bssid = if bssid.trim().is_empty() {
            "0".to_owned()
        } else {
            bssid.trim().to_ascii_lowercase()
        };
        let payload = json!({
            "moduleID": class.module.id,
            "venue": class.venue,
            "courseType": class.module.course_type,
            "courseYear": class.module.academic_year,
            "classDate": date_to_wire(class.date),
            "startTime": (class.start.hour() * 100 + class.start.minute()) as i64,
            "MACaddress": bssid,
            "studentID": self.student_id(),
            "deviceUID": device_uid,
        });
        let result = self.call_function("signAttendance", payload).await?;

        Ok(CheckinOutcome::from_status(status(&result)))
    }

    async fn confirm_registration(&self, key: &str) -> Result<Option<i64>> {
        for _ in 0..3 {
            let bearer = self.bearer().await?;
            if let Some(document) =
                firestore::get_document(&self.http, &bearer, &format!("write_result/{key}")).await?
            {
                return document
                    .get_i64("statusCode")
                    .map(Some)
                    .ok_or(Error::MissingField("statusCode"));
            }

            tokio::time::sleep(Duration::from_millis(700)).await;
        }

        Ok(None)
    }

    async fn call_function(&self, function: &'static str, data: Value) -> Result<Value> {
        let response = self
            .http
            .post(format!(
                "{}/{function}",
                crate::config::functions_base_url()
            ))
            .bearer_auth(self.bearer().await?)
            .json(&json!({"data": data}))
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(Error::function_response(function, status, &body));
        }

        let body: Value = serde_json::from_str(&body)?;
        if let Some(error) = body.get("error") {
            return Err(Error::FunctionFailed {
                function,
                message: error_message(error).unwrap_or_else(|| "unknown error".to_owned()),
            });
        }

        body.get("result")
            .cloned()
            .ok_or(Error::MissingField("result"))
    }
}

impl CheckinOutcome {
    fn from_status(status_code: i64) -> Self {
        match status_code {
            200 => Self::Accepted,
            202 => Self::AcceptedPending,
            401 => Self::Unauthorized,
            403 => Self::DeviceNotInVenue,
            409 => Self::DeviceNotRegistered,
            status_code => Self::Unknown { status_code },
        }
    }
}

fn registration_result(key: String, original: Error, result: Result<Option<i64>>) -> Result<()> {
    match result {
        Ok(Some(200)) => Ok(()),
        Ok(Some(code)) => Err(Error::FunctionStatus {
            function: "registerDeviceSuccess",
            status: code,
        }),
        Ok(None) => Err(Error::DeviceRegistrationUnknown {
            result_key: key,
            source: Box::new(original),
        }),
        Err(error) => Err(Error::DeviceRegistrationUnknown {
            result_key: key,
            source: Box::new(error),
        }),
    }
}

fn status(value: &Value) -> i64 {
    value.get("statusCode").and_then(Value::as_i64).unwrap_or(0)
}

fn error_message(error: &Value) -> Option<String> {
    error
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| error.as_str().map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_requires_positive_confirmation() {
        let original = || Error::MissingField("result");
        assert!(registration_result("key".into(), original(), Ok(Some(200))).is_ok());
        assert!(matches!(
            registration_result("key".into(), original(), Ok(Some(403))),
            Err(Error::FunctionStatus { status: 403, .. })
        ));
        assert!(matches!(
            registration_result("key".into(), original(), Ok(None)),
            Err(Error::DeviceRegistrationUnknown { .. })
        ));
        assert!(matches!(
            registration_result("key".into(), original(), Err(original())),
            Err(Error::DeviceRegistrationUnknown { .. })
        ));
    }
}
