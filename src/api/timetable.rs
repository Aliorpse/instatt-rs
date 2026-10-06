use chrono::{Datelike, Duration, NaiveDate};
use serde_json::json;

use super::firestore;
use crate::{
    client::InstattClient,
    error::Result,
    model::{Class, date_to_wire},
};

impl InstattClient {
    pub async fn classes_between(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<Class>> {
        let query = json!({
            "from": [{"collectionId": "classes"}],
            "where": {
                "compositeFilter": {
                    "op": "AND",
                    "filters": [
                        date_filter(
                            "GREATER_THAN_OR_EQUAL",
                            date_to_wire(from),
                        ),
                        date_filter(
                            "LESS_THAN_OR_EQUAL",
                            date_to_wire(to),
                        ),
                    ],
                },
            },
        });

        self.query_classes(query).await
    }

    pub async fn classes_on(&self, date: NaiveDate) -> Result<Vec<Class>> {
        self.classes_between(date, date).await
    }

    pub async fn all_classes(&self) -> Result<Vec<Class>> {
        self.query_classes(json!({
            "from": [{"collectionId": "classes"}],
        }))
        .await
    }

    pub async fn today(&self) -> Result<Vec<Class>> {
        let date = self.server_time().await?.date;

        self.classes_on(date).await
    }

    pub async fn this_week(&self) -> Result<Vec<Class>> {
        let date = self.server_time().await?.date;
        let monday = date - Duration::days(date.weekday().num_days_from_monday() as i64);

        self.classes_between(monday, monday + Duration::days(6))
            .await
    }

    async fn query_classes(&self, query: serde_json::Value) -> Result<Vec<Class>> {
        let bearer = self.bearer().await?;
        let mut classes: Vec<_> = firestore::run_query(
            &self.http,
            Some(&bearer),
            &format!("students/{}", self.student_id()),
            query,
        )
        .await?
        .iter()
        .map(Class::from_document)
        .collect::<Result<_>>()?;

        classes.sort_by(|a, b| {
            a.date
                .cmp(&b.date)
                .then(a.start.cmp(&b.start))
                .then(a.module.id.cmp(&b.module.id))
        });

        Ok(classes)
    }
}

fn date_filter(operation: &str, date: i32) -> serde_json::Value {
    json!({
        "fieldFilter": {
            "field": {"fieldPath": "classDate"},
            "op": operation,
            "value": {"integerValue": date.to_string()},
        },
    })
}
