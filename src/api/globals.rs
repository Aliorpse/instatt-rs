use serde_json::json;

use super::firestore;
use crate::{
    client::InstattClient,
    error::Result,
    model::{ClassStatusCodes, Globals, date_from_wire, time_from_wire},
};

impl InstattClient {
    /// Fetches global/time on every call. Missing or invalid clock data is an error.
    pub async fn server_time(&self) -> Result<crate::model::ServerTime> {
        use crate::Error;

        let bearer = self.bearer().await?;
        let document = firestore::get_document(&self.http, &bearer, "global/time")
            .await?
            .ok_or(Error::MissingField("global/time"))?;
        Ok(crate::model::ServerTime {
            date: date_from_wire(
                document
                    .get_i64("serverDate")
                    .ok_or(Error::MissingField("serverDate"))?,
            )?,
            time: time_from_wire(
                document
                    .get_i64("serverTime")
                    .ok_or(Error::MissingField("serverTime"))?,
            )?,
        })
    }

    pub async fn globals(&self) -> Result<Globals> {
        if let Some(value) = self.globals.read().await.clone() {
            return Ok(value);
        }
        self.refresh_globals().await
    }

    pub async fn refresh_globals(&self) -> Result<Globals> {
        let bearer = self.bearer().await?;
        let documents = firestore::run_query(
            &self.http,
            Some(&bearer),
            "",
            json!({
                "from": [{"collectionId": "global"}],
            }),
        )
        .await?;
        let mut globals = Globals::default();
        for document in documents {
            match document.id() {
                "classType" => {
                    for (key, value) in document.fields() {
                        if let (Ok(key), Some(value)) = (key.parse(), value.as_str()) {
                            globals.class_types.insert(key, value.to_owned());
                        }
                    }
                }
                "classStatus" => {
                    globals.class_status = ClassStatusCodes {
                        upcoming: document.get_i64("YTBC"),
                        cancelled: document.get_i64("cancelled"),
                        conducted: document.get_i64("conducted"),
                    };
                }
                "courseType" => {
                    globals.course_year = document.get_str("courseYear").map(str::to_owned);
                }
                "absentThreshold" => {
                    globals.absent_threshold = document
                        .get_i64("absentThreshold")
                        .and_then(|value| value.try_into().ok());
                }
                "ssidFilters" => {
                    globals.ssid_filters = document.fields().keys().cloned().collect();
                    globals.ssid_filters.sort();
                }
                _ => {}
            }
        }
        *self.globals.write().await = Some(globals.clone());
        Ok(globals)
    }
}
