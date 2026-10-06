use serde_json::json;

use super::firestore;
use crate::{
    client::InstattClient,
    error::{Error, Result},
    model::{AttendanceSummary, ModuleAttendance, ModuleKey},
};

impl InstattClient {
    /// Reads enabled modules for exactly the supplied backend academic year.
    pub async fn module_attendance(&self, academic_year: &str) -> Result<Vec<ModuleAttendance>> {
        let bearer = self.bearer().await?;
        let parent = format!("students/{}", self.student_id());
        let query = json!({
            "from": [{"collectionId": "modules"}],
            "where": {
                "fieldFilter": {
                    "field": {"fieldPath": "courseYear"},
                    "op": "EQUAL",
                    "value": {"stringValue": academic_year},
                },
            },
        });

        let student_modules: Vec<_> =
            firestore::run_query(&self.http, Some(&bearer), &parent, query)
                .await?
                .into_iter()
                .filter(|document| document.get_i64("enableStatus").unwrap_or(1) != 0)
                .collect();
        let paths: Vec<_> = student_modules
            .iter()
            .map(|document| format!("modules/{}", document.id()))
            .collect();
        let module_documents = firestore::batch_get(&self.http, &bearer, &paths).await?;

        student_modules
            .iter()
            .zip(module_documents.iter())
            .map(|(student, module)| {
                let key = ModuleKey::parse(student.id())?;
                Ok(ModuleAttendance {
                    module: key,
                    title: student.get_str("moduleName").unwrap_or_default().to_owned(),
                    attended: count(student.get_i64("totalAttended"), "totalAttended")?,
                    voided: count(student.get_i64("classVoid"), "classVoid")?,
                    pending_voids: count(student.get_i64("classVoidPending"), "classVoidPending")?,
                    conducted: count(
                        module
                            .as_ref()
                            .and_then(|doc| doc.get_i64("totalConductedClasses")),
                        "totalConductedClasses",
                    )?,
                    class_limit: optional_count(
                        module
                            .as_ref()
                            .and_then(|doc| doc.get_i64("totalClassConstant")),
                        "totalClassConstant",
                    )?,
                })
            })
            .collect()
    }

    /// Aggregates adjusted attendance statistics for one academic year.
    pub async fn attendance_summary(&self, academic_year: &str) -> Result<AttendanceSummary> {
        Ok(AttendanceSummary::from_modules(
            self.module_attendance(academic_year).await?,
        ))
    }

    pub async fn absences(&self) -> Result<Vec<crate::model::Class>> {
        let globals = self.globals().await?;
        let bearer = self.bearer().await?;
        let query = json!({
            "from": [{"collectionId": "classes"}],
            "where": {
                "fieldFilter": {
                    "field": {"fieldPath": "attended"},
                    "op": "IN",
                    "value": {
                        "arrayValue": {
                            "values": [
                                {"integerValue": "0"},
                                {"integerValue": "2"},
                            ],
                        },
                    },
                },
            },
        });

        firestore::run_query(
            &self.http,
            Some(&bearer),
            &format!("students/{}", self.student_id()),
            query,
        )
        .await?
        .iter()
        .map(crate::model::Class::from_document)
        .filter_map(|result| match result {
            Ok(class) if class.status(&globals) == crate::model::ClassStatus::Conducted => {
                Some(Ok(class))
            }
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
    }
}

fn count(value: Option<i64>, field: &'static str) -> Result<u32> {
    value.ok_or(Error::MissingField(field)).and_then(|value| {
        value.try_into().map_err(|_| Error::InvalidField {
            field,
            message: "must be non-negative",
        })
    })
}

fn optional_count(value: Option<i64>, field: &'static str) -> Result<Option<u32>> {
    value
        .map(|value| {
            value.try_into().map_err(|_| Error::InvalidField {
                field,
                message: "must be non-negative",
            })
        })
        .transpose()
}
